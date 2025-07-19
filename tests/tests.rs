
mod common;

#[cfg(test)]
mod tests {
    

    use std::collections::BTreeMap;

    use actix_web::web::Data;
    use futures::FutureExt;    
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
    use kube::{api::{ObjectMeta, PostParams}, Api, CustomResourceExt};
    use opendcs_controllers::{api::{self, v1::tsdb::database::{OpenDcsDatabase, OpenDcsDatabaseSpec}}, schema::controller, telemetry::state::State};
    use rstest::rstest;

    use crate::common::{database::tests::create_postgres_instance, tests::{k8s_info, K8s}};
 

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
        println!("done, attempting to create instance");
        let db_secret = create_postgres_instance(client.clone()).await.expect("Postgres Instance unable to start");
        let odcs_api: Api<OpenDcsDatabase> = Api::namespaced(client.clone(), "default");
        let odcs_database = OpenDcsDatabase {
            metadata: ObjectMeta {
                name: Some("testdb".into()),
                 ..Default::default()
                },
                spec: 
                    OpenDcsDatabaseSpec { 
                        schema_version: "ghcr.io/opendcs/compdepends:main-nightly".into(), 
                        database_secret: db_secret.secret_name.clone(), 
                        placeholders: BTreeMap::from([
                            ("NUM_TS_TABLES".into(),"1".into()),
                            ("NUM_TEXT_TABLES".into(),"1".into())
                        ])
                    }
                ,
                status: None 
            };
             
        odcs_api.create(&pp, &odcs_database).await.expect("Unable to create OpenDCS Database Instance.");
            
        


        controller.now_or_never();
    }

    
}