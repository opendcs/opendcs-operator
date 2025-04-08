#![allow(unused_imports, unused_variables)]
use std::{io::{self, ErrorKind}, sync::Arc, time::Duration};
use actix_web::{get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder};
use api::v1::{dds_recv::DdsConnection, lrgs::{LrgsCluster, LrgsClusterStatus}};
use futures::StreamExt;
use jsonptr::diagnostic;
use k8s_openapi::api::{apps::v1::StatefulSet, core::v1::{ConfigMap, Secret, Service}};
use kube::{api::{Patch, PatchParams}, config::InferConfigError, runtime::{controller::Action, reflector::ObjectRef, watcher, Controller}, Api, Client, Error, Resource, ResourceExt};
use lrgs::{config::{create_lrgs_config, create_managed_users}, configmap::created_script_config_map, controller, service::create_service, state::{Context, State}, statefulset::create_statefulset};
use serde::Serialize;
use serde_json::{json, Value};

mod api;
mod lrgs;


#[tokio::main]
async fn main() ->  anyhow::Result<()> {
    lrgs::telemetry::init().await;
    //let args = Cli::parse();
    // Infer the runtime environment and try to create a Kubernetes Client
    


    let state = State::default();
    let controller = controller::run(state.clone());
    let server = HttpServer::new(move || {
            App::new()
                .app_data(Data::new(state.clone()))
                .wrap(middleware::Logger::default().exclude("/health"))
                .service(index)
                .service(health)
                .service(metrics)
    })
    .bind("0.0.0.0:8080")?
    .shutdown_timeout(5);

    tokio::join!(controller, server.run()).1?;
    Ok(())
}






#[get("/metrics")]
async fn metrics(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let metrics = c.metrics();
    HttpResponse::Ok()
        .content_type("application/openmetrics-text; version=1.0.0; charset=utf-8")
        .body(metrics)
}

#[get("/health")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[get("/")]
async fn index(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}

