#[cfg(test)]
pub mod test {
    use kube::{
        api::{DeleteParams, ListParams, PostParams},
        runtime::wait::await_condition,
        Api, Client,
    };
    use opendcs_controllers::api::v1::tsdb::database::{OpenDcsDatabase, OpenDcsDatabaseSpec};
    use std::collections::BTreeMap;
    use tracing::info;

    use crate::common::database::tests::{odcs_database_ready, PostgresInstance};

    pub struct OpenDcsTestDatabase {
        client: Client,
        name: String,
        _opendcs_database: OpenDcsDatabase,
    }

    impl OpenDcsTestDatabase {
        pub async fn new(
            client: Client,
            name: &str,
            db: &PostgresInstance,
            migration_image: &str,
        ) -> Self {
            let pp = PostParams::default();
            let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(client.clone());
            let test_db_name = name;
            let opendcs_database = OpenDcsDatabase::new(
                test_db_name,
                OpenDcsDatabaseSpec {
                    schema_version: migration_image.into(),
                    database_secret: db.secret_name.clone(),
                    placeholders: BTreeMap::from([
                        ("NUM_TS_TABLES".into(), "1".into()),
                        ("NUM_TEXT_TABLES".into(), "1".into()),
                    ]),
                },
            );
            let _ = odcs_api
                .list(&ListParams::default())
                .await
                .expect("can't list instances");
            odcs_api
                .create(&pp, &opendcs_database)
                .await
                .expect("Unable to create OpenDCS Database Instance.");
            info!("waiting for odcs db ready.");
            let establish = await_condition(odcs_api.clone(), test_db_name, odcs_database_ready());
            let _ = tokio::time::timeout(std::time::Duration::from_secs(300), establish)
                .await
                .expect("database not created");
            Self {
                client: client.clone(),
                name: name.to_string(),
                _opendcs_database: opendcs_database,
            }
        }

        pub async fn delete(&self) -> bool {
            let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(self.client.clone());
            let result = odcs_api.delete(&self.name, &DeleteParams::default()).await;
            return result.is_ok();
        }
    }
}
