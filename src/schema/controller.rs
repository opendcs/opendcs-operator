use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use chrono::Utc;
use futures::StreamExt;
use k8s_openapi::api::{
    batch::v1::{Job, JobSpec},
    core::v1::{ConfigMap, Container, PodSpec, PodTemplateSpec, Secret, SecurityContext, Service},
};
use kube::{
    api::{ObjectMeta, Patch, PatchParams, PostParams},
    runtime::{controller::Action, reflector::ObjectRef, watcher, Controller},
    Api, Client, Error, Resource, ResourceExt,
};
use opendcs_controllers::{
    api::v1::tsdb::database::{MigrationState, OpenDcsDatabase, OpenDcsDatabaseStatus},
    telemetry::{
        state::{Context, State},
        telemetry,
    },
};
use serde_json::json;
use tracing::{error, field, info, instrument, warn, Span};

use crate::job::{self, MigrationJob};

pub async fn run(state: State<OpenDcsDatabase>) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let databases: Api<OpenDcsDatabase> = Api::all(client.clone());
    let jobs: Api<Job> = Api::all(client.clone());

    println!("Starting controller");
    Controller::new(databases.clone(), watcher::Config::default())
         .owns(jobs, watcher::Config::default())
    //     .owns(secrets.clone(), watcher::Config::default())
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
    object: Arc<OpenDcsDatabase>,
    ctx: Arc<Context<OpenDcsDatabase>>,
) -> Result<Action, Error> {
    let trace_id = telemetry::get_trace_id();
    if trace_id != opentelemetry::trace::TraceId::INVALID {
        Span::current().record("trace_id", field::display(&trace_id));
    }
    let _timer = ctx.metrics.reconcile.count_and_measure(&trace_id);
    ctx.diagnostics.write().await.last_event = Utc::now();
    let oref = object.controller_owner_ref(&()).unwrap();
    let client = &ctx.client;
    let name = object.metadata.name.clone().unwrap();
    let ns = object
        .metadata
        .namespace
        .clone()
        .unwrap_or("default".to_string());
    info!("Processing \"{}\" in {}", object.name_any(), ns);
    let databases: Api<OpenDcsDatabase> = Api::namespaced(client.clone(), &ns);
    let jobs: Api<Job> = Api::namespaced(client.clone(), &ns);
    let secret = &object.spec.database_secret;
    let patch_name = "database-controller";

    let migration = MigrationJob::from(&object, client).await;
    let (old_state, new_state) = 
        migration.reconcile().await.expect("No state update provided.");
    
    if old_state.is_none_or(|os| os != new_state) {    
        let new_status = Patch::Apply(json!({
            "apiVersion": "tsdb.opendcs.org/v1",
            "kind": "OpenDcsDatabase",
            "status": OpenDcsDatabaseStatus {
                last_updated: Some(Utc::now()),
                // TODO: wait until actually applied
                applied_schema_version: None,
                state: Some(new_state),
                }
        }));
        
        
        let pp = PatchParams::apply(patch_name);
        databases.patch_status(&name, &pp, &new_status).await?;
    }
    Ok(Action::requeue(Duration::from_secs(3600 / 2)))
}

fn error_policy(
    object: Arc<OpenDcsDatabase>,
    err: &kube::Error,
    ctx: Arc<Context<OpenDcsDatabase>>,
) -> Action {
    warn!("reconcile failed: {:?}", err);
    let e = anyhow!("Api error {:?}", err);
    ctx.metrics.reconcile.set_failure(&object, &e);
    Action::requeue(Duration::from_secs(5 * 60))
}
