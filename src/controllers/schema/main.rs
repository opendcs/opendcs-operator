use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use kube::Client;
use opendcs_controllers::api::v1::tsdb::database::OpenDcsDatabase;
use opendcs_controllers::telemetry::state::State;
use opendcs_controllers::telemetry::telemetry;

use opendcs_controllers::schema::controller;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init().await;
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let state: State<OpenDcsDatabase> = State::default();
    let data = Data::new(state.clone());
    let controller = controller::run(state.clone(),client);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(index)
            .service(health)
            .service(metrics)
    })
    .workers(5)
    .bind("0.0.0.0:8080")?
    .shutdown_timeout(5);

    tokio::join!(controller, server.run()).1?;
    Ok(())
}

#[get("/metrics")]
async fn metrics(c: Data<State<OpenDcsDatabase>>, _req: HttpRequest) -> impl Responder {
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
async fn index(c: Data<State<OpenDcsDatabase>>, _req: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}