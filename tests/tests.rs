
mod common;



#[cfg(test)]
mod tests {
    

    use std::{collections::BTreeMap, thread};

    use actix_web::web::Data;
    use k8s_openapi::{apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition};
    use kube::{api::{ListParams, ObjectMeta, Patch, PatchParams, PostParams}, runtime::{conditions, wait::{await_condition, Condition}}, Api, CustomResourceExt};
    use opendcs_controllers::{api::{self, constants::TSDB_GROUP, v1::tsdb::database::{MigrationState, OpenDcsDatabase, OpenDcsDatabaseSpec}}, schema::controller, telemetry::state::State};
    use rstest::rstest;
    use tokio::{runtime::Handle, task::futures, time::sleep};
   
    use crate::common::{database::tests::create_postgres_instance, tests::{k8s_info, K8s}};
 

    #[rstest]
    #[tokio::test]
    async fn test_simple_migration(#[future] k8s_info: &K8s) {
        let client = k8s_info.await.get_client();
        println!("got client");
        let state: State<OpenDcsDatabase> = State::default();
        let _data = Data::new(state.clone());
        
        let controller = controller::run(state.clone(),client.clone());
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(controller);
        });
        sleep(std::time::Duration::from_secs(5)).await;
        println!("getting crd api");
        let crd_api: Api<CustomResourceDefinition> = Api::all(client.clone());
        let patch = PatchParams::apply("odcs db test").force();
        let pp = PostParams::default();
        println!("applying crd");
        let crd_name = format!("opendcsdatabases.{}", &TSDB_GROUP);
        crd_api.patch(&crd_name,&patch, &Patch::Apply(OpenDcsDatabase::crd())).await.expect("can't make database crd.");
        let establish = await_condition(crd_api, &crd_name, conditions::is_crd_established());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), establish).await.expect("crd not successfully loaded");
        println!("done, attempting to create instance");
        let db_secret = create_postgres_instance(client.clone()).await.expect("Postgres Instance unable to start");
        let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(client.clone());
        let odcs_database = OpenDcsDatabase::new("testdb",
                    OpenDcsDatabaseSpec { 
                        schema_version: "ghcr.io/opendcs/compdepends:main-nightly".into(), 
                        database_secret: db_secret.secret_name.clone(), 
                        placeholders: BTreeMap::from([
                            ("NUM_TS_TABLES".into(),"1".into()),
                            ("NUM_TEXT_TABLES".into(),"1".into())
                        ])
                    }
                );
        let _ = odcs_api.list(&ListParams::default()).await.expect("can't list instances");
        odcs_api.create(&pp, &odcs_database).await.expect("Unable to create OpenDCS Database Instance.");
        println!("waiting for odcs db ready.");
        let name = "testdb";
        let establish = await_condition(odcs_api, name, odcs_database_ready());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(30), establish).await.expect("database not created");
        //controller.now_or_never();
    }

    
   

    fn odcs_database_ready() -> impl Condition<OpenDcsDatabase> {
        move |obj: Option<&OpenDcsDatabase>| {
            if let Some(db) = &obj {
                if let Some(status) = &db.status {
                    if let Some(state) = &status.state {
                        return *state == MigrationState::Ready
                    }
                }
            }
            false
        }
    }
    
}