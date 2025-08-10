pub mod database;
pub mod opendcs_database;
#[cfg(test)]
pub mod tests {
    use actix_web::web::Data;
    use anyhow::Result;
    use ctor::{ctor, dtor};

    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
    use opendcs_controllers::{
        api::v1::{lrgs::LrgsCluster, tsdb::database::OpenDcsDatabase},
        schema::controller,
        telemetry::state::State,
    };
    use tracing::{debug, warn};
    use tracing_subscriber::{prelude::*, EnvFilter, Registry};

    use std::{
        env,
        process::Command,
        thread::{self, JoinHandle},
    };

    use kube::{
        api::{Patch, PatchParams},
        config::{KubeConfigOptions, Kubeconfig},
        runtime::{conditions, wait::await_condition},
        Api, Client, Config, CustomResourceExt,
    };
    use rstest::fixture;
    use rustls::crypto::CryptoProvider;
    use tokio::sync::OnceCell;

    use crate::common::database::tests::{create_postgres_instance, PostgresInstance};

    pub struct K8s {
        client: Client,
        _schema_controller: JoinHandle<()>,
    }

    impl K8s {
        pub async fn new() -> K8s {
            let result = Command::new("sh")
                .args(["-c", "kind create cluster --name odcs-test"])
                .output()
                .expect("Failed to start kind");
            debug!("{result:?}");
            let kconfig = Kubeconfig::read().expect("unable to read any kubernetes config files");
            let opts = KubeConfigOptions {
                // kind prefixes everything with kind-
                cluster: Some("kind-odcs-test".into()),
                ..Default::default()
            };
            let config = Config::from_custom_kubeconfig(kconfig, &opts)
                .await
                .expect("Unable to create config.");
            let client = Client::try_from(config).expect("Unable to create client");
            let schema_controller = K8s::start_schema_controller(client.clone());
            let inst = K8s {
                client: client,
                _schema_controller: schema_controller,
            };
            inst.load_crds()
                .await
                .expect("Unable to load CRD definitions.");
            return inst;
        }

        pub fn get_client(&self) -> Client {
            self.client.clone()
        }

        fn start_schema_controller(client: Client) -> JoinHandle<()> {
            let state: State<OpenDcsDatabase> = State::default();
            let _data = Data::new(state.clone());

            let controller = controller::run(state.clone(), client.clone());
            let schema_thread = thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(controller);
            });
            schema_thread
        }

        async fn load_crds(&self) -> Result<()> {
            let crd_api: Api<CustomResourceDefinition> = Api::all(self.client.clone());
            let patch = PatchParams::apply("odcs db test").force();

            debug!("Loading CRDs");
            let crd_name = OpenDcsDatabase::crd_name();
            crd_api
                .patch(&crd_name, &patch, &Patch::Apply(OpenDcsDatabase::crd()))
                .await
                .expect("can't make database crd.");
            let establish =
                await_condition(crd_api.clone(), &crd_name, conditions::is_crd_established());
            let _ = tokio::time::timeout(std::time::Duration::from_secs(10), establish)
                .await
                .expect("crd not successfully loaded");

            let crd_name = LrgsCluster::crd_name();
            crd_api
                .patch(&crd_name, &patch, &Patch::Apply(LrgsCluster::crd()))
                .await
                .expect("can't make database crd.");

            let establish =
                await_condition(crd_api.clone(), &crd_name, conditions::is_crd_established());
            let _ = tokio::time::timeout(std::time::Duration::from_secs(10), establish)
                .await
                .expect("crd not successfully loaded");

            debug!("CRDs loaded an established.");
            Ok(())
        }

        pub async fn create_database(&self, name: &str) -> PostgresInstance {
            create_postgres_instance(self.client.clone(), name)
                .await
                .expect("Unable to create posgres instance.")
        }
    }

    static K8S_INST: OnceCell<K8s> = OnceCell::const_new();

    #[fixture]
    //#[once]
    pub async fn k8s_inst() -> &'static K8s {
        K8S_INST.get_or_init(|| async { K8s::new().await }).await
    }

    #[ctor]
    fn on_startup() {
        if CryptoProvider::get_default().is_none() {
            rustls::crypto::ring::default_provider()
                .install_default()
                .expect("Error initializing rustls provider");
        }

        #[cfg(feature = "telemetry")]
        let otel = tracing_opentelemetry::OpenTelemetryLayer::new(init_tracer());

        let logger = tracing_subscriber::fmt::layer().compact();
        let env_filter = EnvFilter::try_from_default_env()
            .or(EnvFilter::try_new("info"))
            .unwrap();

        // Decide on layers
        let reg = Registry::default();
        #[cfg(feature = "telemetry")]
        reg.with(env_filter).with(logger).with(otel).init();
        #[cfg(not(feature = "telemetry"))]
        reg.with(env_filter).with(logger).init();
    }

    #[dtor]
    fn on_shutdown() {
        let keep = env::var("KEEP_KIND").is_ok_and(|s| s == "true");
        if !keep {
            let result = Command::new("sh")
                .args(["-c", "kind delete cluster --name odcs-test"])
                .output()
                .expect("Failed to start kind");
            debug!("{result:?}");
        } else {
            warn!("Kind cluster was not removed, you may need to perform manual cleanup before next run.");
        }
    }
}
