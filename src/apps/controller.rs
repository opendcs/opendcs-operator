use std::{sync::Arc, time::Duration};

use crate::{
    api::v1::tsdb::{
        app::{OpenDcsApp, OpenDcsAppSpec},
        database::{MigrationState, OpenDcsDatabase},
    },
    apps::app_deployment::from,
    telemetry::{
        state::{Context, State},
        telemetry,
    },
};
use anyhow::anyhow;
use chrono::Utc;
use futures::StreamExt;
use k8s_openapi::api::{apps::v1::Deployment, core::v1::Secret};
use kube::{
    Api, Client, Error, ResourceExt,
    api::{Patch, PatchParams},
    runtime::{Controller, controller::Action, watcher},
};

use tracing::{Span, field, info, instrument, warn};

pub async fn run(state: State<OpenDcsApp>, client: Client) {
    let apps: Api<OpenDcsApp> = Api::all(client.clone());
    let deployments: Api<Deployment> = Api::all(client.clone());
    let secrets: Api<Secret> = Api::all(client.clone());
    println!("Starting DcsApp controller");
    Controller::new(apps.clone(), watcher::Config::default())
        .owns(deployments, watcher::Config::default())
        .owns(secrets.clone(), watcher::Config::default())
        //     .owns(services.clone(), watcher::Config::default())
        //     .owns(cm, watcher::Config::default())
        //     .watches(secrets.clone(), user_watch_config, user_mapper)
        //     .watches(
        //         dds_connections.clone(),
        //         watcher::Config::default(),
        //         dds_mapper,
        //     )
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.to_context(client).await)
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[instrument(skip(object, ctx), fields(trace_id))]
async fn reconcile(
    object: Arc<OpenDcsApp>,
    ctx: Arc<Context<OpenDcsApp>>,
) -> Result<Action, Error> {
    let trace_id = telemetry::get_trace_id();
    if trace_id != opentelemetry::trace::TraceId::INVALID {
        Span::current().record("trace_id", field::display(&trace_id));
    }
    let _timer = ctx.metrics.reconcile.count_and_measure(&trace_id);
    ctx.diagnostics.write().await.last_event = Utc::now();
    let client = &ctx.client;
    let ns = object
        .metadata
        .namespace
        .clone()
        .unwrap_or("default".to_string());
    info!("Processing \"{}\" in {}", object.name_any(), ns);
    let apps: Api<OpenDcsApp> = Api::namespaced(client.clone(), &ns);
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), &ns);
    let _secrets: Api<Secret> = Api::namespaced(client.clone(), &ns);
    let databases: Api<OpenDcsDatabase> = Api::namespaced(client.clone(), &ns);
    let patch_name = "app-controller";

    let db_name = &object.spec.database;
    let db = match databases.get_opt(&db_name).await? {
        Some(db) => db,
        None => {
            info!("OpenDcsDatabase {} does not yet exist", db_name);
            return Ok(Action::requeue(Duration::from_secs(3600 / 4)));
        }
    };
    let db_status = match &db.status {
        Some(s) => s.state.clone().unwrap_or(MigrationState::Fresh),
        None => {
            return Ok(Action::requeue(Duration::from_secs(3600 / 4)));
        }
    };

    let app = apps.get_opt(&object.spec.application).await?;

    match db_status {
        MigrationState::Fresh => {
            return Ok(Action::requeue(Duration::from_mins(5)));
        }
        MigrationState::PreparingToMigrate => {
            match app {
                Some(app) => {
                    info!("Database is preparing to migrate, bringing replicas to 0");
                    let updated_app = OpenDcsApp {
                        spec: OpenDcsAppSpec {
                            replicas: Some(0),
                            ..app.spec
                        },
                        ..app
                    };
                    // Update the deployment to set to 0
                    let deployment = from(&updated_app, &db, client).await;
                    let pp = PatchParams::apply(patch_name);
                    deployments
                        .patch(&deployment.name_any(), &pp, &Patch::Apply(deployment))
                        .await?;
                }
                None => {
                    // no app yet, don't worry about it.
                    return Ok(Action::requeue(Duration::from_mins(5)));
                }
            }
        }
        MigrationState::Migrating => {
            info!(
                "Database {} is currently migrating, waiting for completion.",
                db_name
            );
            return Ok(Action::requeue(Duration::from_mins(5)));
        }
        MigrationState::Ready => {
            info!("Database ready, creating/updating deployment");
            let deployment = from(&object, &db, client).await;
            let pp = PatchParams::apply(patch_name);
            deployments
                .patch(&deployment.name_any(), &pp, &Patch::Apply(deployment))
                .await?;
        }
        MigrationState::Failed => {
            info!(
                "Database {} is in a failed state. Waiting until until fixed to change anything.",
                db_name
            );
            return Ok(Action::requeue(Duration::from_mins(30)));
        }
    }

    // if db preparinging to migrate bring counts to 0
    // if db migrating, make sure nothing else starts

    Ok(Action::requeue(Duration::from_secs(3600 / 2)))
}

fn error_policy(
    object: Arc<OpenDcsApp>,
    err: &kube::Error,
    ctx: Arc<Context<OpenDcsApp>>,
) -> Action {
    warn!("reconcile failed: {:?}", err);
    let e = anyhow!("Api error {:?}", err);
    ctx.metrics.reconcile.set_failure(&object, &e);
    Action::requeue(Duration::from_secs(5 * 60))
}
