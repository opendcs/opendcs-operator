#[cfg(test)]
mod tests {
    


    // Note this useful idiom: importing names from outer (for mod tests) scope.
    

    use ctor::dtor;
    use std::{future::Future, time::Duration};

    use actix_web::web::Data;
    use futures::{executor::block_on, FutureExt};
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
    use kube::{api::PostParams, config::{KubeConfigOptions, Kubeconfig}, Api, Client, Config, CustomResourceExt};
    use opendcs_controllers::{api::{self, v1::tsdb::database::OpenDcsDatabase}, telemetry::state::State};
    use rstest::{fixture, rstest};
    use testcontainers_modules::{
        k3s::{K3s, KUBE_SECURE_PORT},
        testcontainers::{core::logs::LogFrame, runners::AsyncRunner, Container, ContainerAsync, ImageExt},
    };
    use rustls::crypto::CryptoProvider;
    use tokio::sync::OnceCell;

    use crate::controller;

    pub struct K8s {
        client: Client,
        _instance: ContainerAsync<K3s>
    }

    impl K8s {
        pub async fn new() -> K8s {
            use std::env::temp_dir;

            let k3s_path = &temp_dir();
            println!("starting k3s");
            let k3s_instance = K3s::default()
                .with_conf_mount(k3s_path)
                .with_privileged(true)
                .with_userns_mode("host")
                //.with_startup_timeout(Duration::from_secs(60))
                .start().await
                .unwrap();
            println!("getting client");
            let client = get_kube_client(&k3s_instance).await.expect("can't get client");
            println!("returning set");
            return K8s {client: client, _instance: k3s_instance };
        }

        pub fn get_client(&self) -> Client {
            self.client.clone()
        }
    }

    
    static K8S_INST: OnceCell<K8s> = OnceCell::const_new();
    

    #[fixture]
    //#[once]
    async fn k8s_info() -> &'static K8s {
        println!("hello!");
        K8S_INST.get_or_init(|| async {
            K8s::new().await
        }).await
        
    }

    #[dtor]
    fn on_shutdown() {
        println!("Stopping k3s");
        async {
            let result = match K8S_INST.get() {
                Some(inst) => inst._instance.stop().await,
                None => Result::Ok(()),
            };
        };
        ()
    }

    #[rstest]
    #[tokio::test]
   async fn test_test(#[future] k8s_info: &K8s) {
    let client = k8s_info.await.get_client();
    println!("got client");
    let state: State<OpenDcsDatabase> = State::default();
    let _data = Data::new(state.clone());
    let controller = controller::run(state.clone(),client.clone());
    println!("getting crd api");
    let crd_api: Api<CustomResourceDefinition> = Api::all(client.clone());
    let pp = PostParams::default();
    println!("applying crd");
    crd_api.create(&pp, &api::v1::tsdb::database::OpenDcsDatabase::crd()).await.expect("can't make database crd.");
    println!("done");
controller.now_or_never();
    }

    // taken from testcontainers-k3s
    //  module test https://docs.rs/crate/testcontainers-modules/latest/source/src/k3s/mod.rs#235
    pub async fn get_kube_client(
        container: &ContainerAsync<K3s>,
    ) -> Result<kube::Client, Box<dyn std::error::Error + 'static>> {
        if CryptoProvider::get_default().is_none() {
            rustls::crypto::ring::default_provider()
                .install_default()
                .expect("Error initializing rustls provider");
        }

        let conf_yaml = container.image().read_kube_config()?;

        let mut config = Kubeconfig::from_yaml(&conf_yaml).expect("Error loading kube config");

        let port = container.get_host_port_ipv4(KUBE_SECURE_PORT).await?;
        config.clusters.iter_mut().for_each(|cluster| {
            if let Some(server) = cluster.cluster.as_mut().and_then(|c| c.server.as_mut()) {
                *server = format!("https://127.0.0.1:{port}")
            }
        });

        let client_config =
            Config::from_custom_kubeconfig(config, &KubeConfigOptions::default()).await?;

        Ok(kube::Client::try_from(client_config)?)
    }
}