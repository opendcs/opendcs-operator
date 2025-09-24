#[cfg(test)]
pub mod test {
    use kube::{
        Api, Client,
        api::{DeleteParams, ListParams, ObjectMeta, Patch, PatchParams},
        runtime::wait::{Condition, await_condition},
    };
    use opendcs_controllers::api::v1::tsdb::database::{
        MigrationState, OpenDcsDatabase, OpenDcsDatabaseSpec,
    };
    use std::collections::BTreeMap;
    use tracing::info;

    use crate::common::database::tests::{
        PostgresInstance, odcs_database_ready, odcs_database_state,
    };

    pub struct OpenDcsTestDatabase {
        client: Client,
        pub name: String,
        pub opendcs_database: OpenDcsDatabase,
    }

    impl OpenDcsTestDatabase {
        pub async fn new(
            client: Client,
            name: &str,
            db: &PostgresInstance,
            migration_image: &str,
        ) -> Self {
            let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(client.clone());
            create(client.clone(), name, migration_image, db).await;

            info!("waiting for odcs db ready.");
            let establish = await_condition(odcs_api.clone(), name, odcs_database_ready());
            let _ = tokio::time::timeout(std::time::Duration::from_secs(300), establish)
                .await
                .expect("database not created");
            let retrieved = odcs_api
                .get(name)
                .await
                .expect("Could not retrieve database we just created.");

            Self {
                client: client.clone(),
                name: name.to_string(),
                opendcs_database: retrieved,
            }
        }

        pub async fn upgrade(
            client: Client,
            name: &str,
            db: &PostgresInstance,
            migration_image: &str,
        ) -> Self {
            let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(client.clone());
            create(client.clone(), name, migration_image, db).await;

            info!("waiting for odcs db to prep.");
            let condition = odcs_database_state(MigrationState::Ready).not();
            let establish = await_condition(odcs_api.clone(), name, condition);
            let _ = tokio::time::timeout(std::time::Duration::from_secs(300), establish)
                .await
                .expect("system not prepped");

            info!("waiting for odcs db ready.");
            let establish = await_condition(odcs_api.clone(), name, odcs_database_ready());
            let _ = tokio::time::timeout(std::time::Duration::from_secs(600), establish)
                .await
                .expect("database not updated");

            let retrieved = odcs_api
                .get(name)
                .await
                .expect("Could not retrieve database we just created.");

            Self {
                client: client.clone(),
                name: name.to_string(),
                opendcs_database: retrieved,
            }
        }

        pub async fn delete(&self) -> bool {
            let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(self.client.clone());
            let result = odcs_api.delete(&self.name, &DeleteParams::default()).await;
            return result.is_ok();
        }
    }

    async fn create(client: Client, name: &str, migration_image: &str, db: &PostgresInstance) {
        let test_db_name = name;
        let pp = PatchParams::apply(name);
        let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(client.clone());

        let opendcs_database = OpenDcsDatabase {
            metadata: ObjectMeta {
                name: Some(test_db_name.into()),
                ..Default::default()
            },
            spec: OpenDcsDatabaseSpec {
                schema_version: migration_image.into(),
                database_secret: db.secret_name.clone(),
                placeholders: BTreeMap::from([
                    ("NUM_TS_TABLES".into(), "1".into()),
                    ("NUM_TEXT_TABLES".into(), "1".into()),
                ]),
            },
            status: None,
        };
        let _ = odcs_api
            .list(&ListParams::default())
            .await
            .expect("can't list instances");
        odcs_api
            .patch(name, &pp, &Patch::Apply(opendcs_database.clone()))
            .await
            .expect("Unable to create OpenDCS Database Instance.");
    }
}
