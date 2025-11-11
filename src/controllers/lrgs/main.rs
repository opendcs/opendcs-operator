use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder, get, middleware, web::Data,
};
use opendcs_controllers::api::v1::lrgs::LrgsCluster;
use opendcs_controllers::lrgs::controller;
use opendcs_controllers::telemetry::state::State;
use opendcs_controllers::telemetry::telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init().await;
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install rustls crypto provider");
    let state: State<LrgsCluster> = State::default();
    let data = Data::new(state.clone());
    let controller = controller::run(state.clone());
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
async fn metrics(c: Data<State<LrgsCluster>>, _req: HttpRequest) -> impl Responder {
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
async fn index(c: Data<State<LrgsCluster>>, _req: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}
