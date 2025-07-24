use std::{sync::Arc, time::Duration};

use crate::{
    api::v1::{
        dds_recv::DdsConnection,
        lrgs::{LrgsCluster, LrgsClusterStatus},
    },
    lrgs::{
        config::{create_lrgs_config, create_managed_users},
        configmap::created_script_config_map,
        service::create_service,
        statefulset::create_statefulset,
    },
    telemetry::{
        state::{Context, State},
        telemetry,
    },
};
use anyhow::anyhow;
use chrono::Utc;
use futures::StreamExt;
use k8s_openapi::api::{
    apps::v1::StatefulSet,
    core::v1::{ConfigMap, Secret, Service},
};
use kube::{
    api::{Patch, PatchParams},
    runtime::{controller::Action, reflector::ObjectRef, watcher, Controller},
    Api, Client, Error, Resource, ResourceExt,
};
use serde_json::json;
use tracing::{error, field, info, instrument, warn, Span};

pub async fn run(state: State<LrgsCluster>) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let secrets: Api<Secret> = Api::all(client.clone());
    let cm: Api<ConfigMap> = Api::all(client.clone());
    let services: Api<Service> = Api::all(client.clone());
    let lrgs_cluster: Api<LrgsCluster> = Api::all(client.clone());
    let dds_connections: Api<DdsConnection> = Api::all(client.clone());
    let stateful_set: Api<StatefulSet> = Api::all(client.clone());
    let user_watch_config =
        watcher::Config::default().fields("type=LrgsCluster.opendcs.org/ddsuser");

    let user_mapper = |obj: Secret| {
        let binding = obj.metadata.labels.unwrap();
        let name = binding.get("lrgs.opendcs.org/lrgs-cluster").unwrap();
        let namespace = obj.metadata.namespace.unwrap_or("default".to_string());
        let obj_ref = ObjectRef::new(name).within(&namespace);
        Some(obj_ref)
    };

    let dds_mapper = |obj: DdsConnection| {
        let binding = obj.metadata.labels.unwrap();
        let name = binding.get("lrgs.opendcs.org/lrgs-cluster").unwrap();
        let namespace = obj.metadata.namespace.unwrap_or("default".to_string());
        let obj_ref = ObjectRef::new(name).within(&namespace);
        Some(obj_ref)
    };

    println!("Starting controller");
    Controller::new(lrgs_cluster.clone(), watcher::Config::default())
        .owns(stateful_set, watcher::Config::default())
        .owns(secrets.clone(), watcher::Config::default())
        .owns(services.clone(), watcher::Config::default())
        .owns(cm, watcher::Config::default())
        .watches(secrets.clone(), user_watch_config, user_mapper)
        .watches(
            dds_connections.clone(),
            watcher::Config::default(),
            dds_mapper,
        )
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.to_context(client).await)
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[instrument(skip(object, ctx), fields(trace_id))]
async fn reconcile(
    object: Arc<LrgsCluster>,
    ctx: Arc<Context<LrgsCluster>>,
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
    let lrgs_api: Api<LrgsCluster> = Api::namespaced(client.clone(), &ns);
    let stateful_api: Api<StatefulSet> = Api::namespaced(client.clone(), &ns);
    let config_map_api: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let secrets_api: Api<Secret> = Api::namespaced(client.clone(), &ns);
    let service_api: Api<Service> = Api::namespaced(client.clone(), &ns);

    let (lrgs_config_map, script_hash) = created_script_config_map(ns.clone(), &oref);
    let lrgs_config = create_lrgs_config(client.clone(), &object, &oref).await;
    if lrgs_config.is_err() {
        let error = lrgs_config.err().unwrap();
        error!("Unable to build Configuration for Lrgs Cluster {}", error);
        //return Err(kube::Error::ReadEvents(io::Error::new(ErrorKind::Other, error.to_string())))  ;
        return Ok(Action::requeue(Duration::from_secs(3600)));
    }
    let lrgs_config = lrgs_config.ok().unwrap();
    let lrgs_config_secret = lrgs_config.secret;

    let lrgs_managed_users = match create_managed_users(client.clone(), &object, &oref).await {
        Ok(lmu) => lmu,
        Err(e) => {
            println!("Unable to process managed users. {:?}", e);
            Vec::new()
        }
    };

    let lrgs_service = create_service(client.clone(), &object, &oref);
    let lrgs_statefulset =
        create_statefulset(&object, lrgs_config.hash.clone(), script_hash.clone());
    let patch_name = "lrgs-controller";
    let serverside = PatchParams::apply(patch_name);
    secrets_api
        .patch(
            &lrgs_config_secret.name_any(),
            &serverside,
            &Patch::Apply(lrgs_config_secret),
        )
        .await?;
    config_map_api
        .patch(
            &lrgs_config_map.name_any(),
            &serverside,
            &Patch::Apply(lrgs_config_map),
        )
        .await?;
    stateful_api
        .patch(
            &lrgs_statefulset.name_any(),
            &serverside,
            &Patch::Apply(lrgs_statefulset),
        )
        .await?;
    for svc in lrgs_service {
        service_api
            .patch(&svc.name_any(), &serverside, &Patch::Apply(svc))
            .await?;
    }

    for user in lrgs_managed_users {
        secrets_api
            .patch(&user.name_any(), &serverside, &Patch::Apply(user))
            .await?;
    }

    if object
        .status
        .as_ref()
        .is_none_or(|lrgs| lrgs.checksum != lrgs_config.hash)
    {
        // always overwrite status object with what we saw
        let new_status = Patch::Apply(json!({
            "apiVersion": "lrgs.opendcs.org/v1",
            "kind": "LrgsCluster",
            "status": LrgsClusterStatus {
                checksum: lrgs_config.hash.clone(),
                last_updated: Some(Utc::now()) }
        }));
        let ps = PatchParams::apply(&patch_name).force();
        let _o = lrgs_api.patch_status(&name, &ps, &new_status).await?;
    }

    Ok(Action::requeue(Duration::from_secs(3600 / 2)))
}

fn error_policy(
    object: Arc<LrgsCluster>,
    err: &kube::Error,
    ctx: Arc<Context<LrgsCluster>>,
) -> Action {
    warn!("reconcile failed: {:?}", err);
    let e = anyhow!("Api error {:?}", err);
    ctx.metrics.reconcile.set_failure(&object, &e);
    Action::requeue(Duration::from_secs(5 * 60))
}
