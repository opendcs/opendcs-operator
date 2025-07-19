
mod common;

#[cfg(test)]
mod tests {
    

    use actix_web::web::Data;
    use futures::FutureExt;    
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
    use kube::{api::PostParams, Api, CustomResourceExt};
    use opendcs_controllers::{api::{self, v1::tsdb::database::OpenDcsDatabase}, schema::controller, telemetry::state::State};
    use rstest::rstest;

    use crate::common::tests::{k8s_info, K8s};
 

    #[rstest]
    #[tokio::test]
    async fn test_simple_migration(#[future] k8s_info: &K8s) {
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
        //let odcs_api: Api<api::v1::tsdb::database::OpenDcsDatabase> = Api::namespaced(client.clone(), "default");
        //odcs_api.create(pp, OpenDcsDatabase { metadata: (), spec: api::v1::tsdb::database::OpenDcsDatabaseSpec { schema_version: (), database_secret: (), placeholders: () }, status: () });
        controller.now_or_never();
    }

    
}