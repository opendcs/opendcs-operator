pub mod database;
#[cfg(test)]
pub mod tests {

    // Note this useful idiom: importing names from outer (for mod tests) scope.

    use ctor::{ctor, dtor};

    use tracing_subscriber::{prelude::*, EnvFilter, Registry};

    use std::process::Command;

    
    use kube::{
        config::{KubeConfigOptions, Kubeconfig},
        Client, Config,
    };
    use rstest::fixture;
    use rustls::crypto::CryptoProvider;
    use tokio::sync::OnceCell;

    pub struct K8s {
        client: Client,
    }

    impl K8s {
        pub async fn new() -> K8s {
            let result = Command::new("sh")
                .args(["-c", "kind create cluster --name odcs-test"])
                .output()
                .expect("Failed to start kind");
            println!("{result:?}");
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
            return K8s { client: client };
        }

        pub fn get_client(&self) -> Client {
            self.client.clone()
        }
    }

    impl Drop for K8s {
        fn drop(&mut self) {
            println!("Stopping kind");
        }
    }

    static K8S_INST: OnceCell<K8s> = OnceCell::const_new();

    #[fixture]
    //#[once]
    pub async fn k8s_info() -> &'static K8s {
        println!("hello!");
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
        let result = Command::new("sh")
            .args(["-c", "kind delete cluster --name odcs-test"])
            .output()
            .expect("Failed to start kind");
        println!("{result:?}");
    }
}
