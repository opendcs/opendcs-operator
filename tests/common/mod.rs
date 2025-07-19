#[cfg(test)]
pub mod tests {
    


    // Note this useful idiom: importing names from outer (for mod tests) scope.
    

    use ctor::dtor;
    use serde::{Deserialize, Serialize};
    use serde_yaml::Value;
    use std::{future::Future, time::Duration};

    use actix_web::web::Data;
    use futures::{executor::block_on, FutureExt};
    use k8s_openapi::{api::{apps::v1::Deployment, core::v1::Pod}, apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition, Resource};
    use kube::{api::{DynamicObject, Object, PostParams}, config::{KubeConfigOptions, Kubeconfig}, discovery, Api, Client, Config, CustomResourceExt};
    use opendcs_controllers::{api::{self, v1::tsdb::database::OpenDcsDatabase}, schema::controller, telemetry::state::State};
    use rstest::{fixture, rstest};
    use testcontainers_modules::{
        k3s::{K3s, KUBE_SECURE_PORT},
        testcontainers::{core::logs::LogFrame, runners::AsyncRunner, Container, ContainerAsync, ImageExt},
    };
    use rustls::crypto::CryptoProvider;
    use tokio::sync::OnceCell;

    

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

    impl Drop for K8s {
        fn drop(&mut self) {
            println!("Stopping k3s");
            let _ = block_on(self._instance.stop());
        }
    }

    
    static K8S_INST: OnceCell<K8s> = OnceCell::const_new();
    

    #[fixture]
    //#[once]
    pub async fn k8s_info() -> &'static K8s {
        println!("hello!");
        K8S_INST.get_or_init(|| async {
            K8s::new().await
        }).await
        
    }

    #[dtor]
    fn on_shutdown() {
     
        //let _= K8S_INST.get_mut();
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