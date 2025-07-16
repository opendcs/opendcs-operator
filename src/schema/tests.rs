#[cfg(test)]
mod tests {
    

    use crate::controller;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    


    use actix_web::web::Data;
    use kube::{config::{KubeConfigOptions, Kubeconfig}, Client, Config};
    use opendcs_controllers::{api::v1::tsdb::database::OpenDcsDatabase, telemetry::state::State};
    use testcontainers_modules::{
        k3s::{K3s, KUBE_SECURE_PORT},
        testcontainers::{runners::AsyncRunner, ImageExt},
    };

    #[tokio::test]
   async fn test_test() {
        use std::env::temp_dir;

let k3s_path = &temp_dir();
let k3s_instance = K3s::default()
    .with_conf_mount(k3s_path)
    .with_privileged(true)
    .with_userns_mode("host")
    .start().await
    .unwrap();

let kube_port = k3s_instance.get_host_port_ipv4(KUBE_SECURE_PORT);
let kube_conf = k3s_instance
    .image()
    .read_kube_config()
    .expect("Cannot read kube conf");

let kconf = Kubeconfig::from_yaml(&kube_conf).expect("cant parse config");
let conf = Config::from_custom_kubeconfig(kconf, &KubeConfigOptions::default()).await.expect("cant make config");
let client = Client::try_from(conf).expect("unable to create client.");
    let state: State<OpenDcsDatabase> = State::default();
    let data = Data::new(state.clone());
    let controller = controller::run(state.clone(),client);
    
    }
}