mod common;

#[cfg(test)]
mod tests {

    use kube::{
        Api,
        api::{ObjectMeta, Patch, PatchParams},
    };
    use opendcs_controllers::api::v1::tsdb::{
        app::{OpenDcsApp, OpenDcsAppSpec},
        database::MigrationState,
    };
    use rstest::rstest;
    use tracing::info;

    use crate::common::{
        opendcs_database::test::OpenDcsTestDatabase,
        tests::{K8s, k8s_inst},
    };

    #[rstest]
    #[tokio::test]
    async fn test_schema_upgrade(#[future] k8s_inst: &K8s) {
        let k8s_inst = k8s_inst.await;
        let client = k8s_inst.get_client();
        let base_image = "ghcr.io/opendcs/migration:main-nightly";
        let upgrade_image = "ghcr.io/opendcs/migration:sha-e1efbba";

        let db = k8s_inst.create_database("upgrade").await;

        let odcs_db =
            OpenDcsTestDatabase::new(client.clone(), "testdb-upgrade", &db, base_image).await;
        let status = odcs_db.opendcs_database.status.expect("No status?");
        assert!(status.state == Some(MigrationState::Ready));
        assert!(status.applied_schema_version == Some(base_image.into()));

        // start a depending application
        // Change the schema image to trigger migration and wait.
        let odcs_db =
            OpenDcsTestDatabase::upgrade(client.clone(), "testdb-upgrade", &db, upgrade_image)
                .await;

        let status = odcs_db.opendcs_database.status.clone().expect("No status?");
        assert!(status.state == Some(MigrationState::Ready));
        assert!(status.applied_schema_version == Some(upgrade_image.into()));

        assert!(odcs_db.delete().await);

        info!("OpenDCS Database Removed.");
        db.close().await.expect("Unable to cleanup resources.");
    }

    #[rstest]
    #[tokio::test]
    async fn test_app_starts(#[future] k8s_inst: &K8s) {
        let k8s_inst = k8s_inst.await;
        let client = k8s_inst.get_client();
        let base_image = "ghcr.io/opendcs/migration:main-nightly";

        let db = k8s_inst.create_database("app").await;

        let odcs_db = OpenDcsTestDatabase::new(client.clone(), "testdb-app", &db, base_image).await;
        let status = odcs_db.opendcs_database.status.clone().expect("No status?");
        assert!(status.state == Some(MigrationState::Ready));
        assert!(status.applied_schema_version == Some(base_image.into()));

        let app = OpenDcsApp {
            metadata: ObjectMeta {
                name: Some("testapp-web-api".into()),
                ..odcs_db.opendcs_database.metadata.clone()
            },
            spec: OpenDcsAppSpec {
                application: "web-api".into(),
                version: Some("main-nightly".into()),
                ..Default::default()
            },
            status: None,
        };
        let pp = PatchParams::apply("testapp-patch");
        let dcs_app_api: Api<OpenDcsApp> = Api::default_namespaced(client.clone());
        dcs_app_api
            .patch("testapp-patch", &pp, &Patch::Apply(app.clone()))
            .await
            .expect("Unable to create OpenDCS App Instance.");

        assert!(odcs_db.delete().await);

        info!("OpenDCS Database Removed.");
        db.close().await.expect("Unable to cleanup resources.");
    }
}
