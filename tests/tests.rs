mod common;

#[cfg(test)]
mod tests {

    use opendcs_controllers::api::v1::tsdb::database::MigrationState;
    use rstest::rstest;
    use tracing::info;

    use crate::common::{
        opendcs_database::test::OpenDcsTestDatabase,
        tests::{k8s_inst, K8s},
    };

    #[rstest]
    #[tokio::test]
    async fn test_simple_migration(#[future] k8s_inst: &K8s) {
        let k8s_inst = k8s_inst.await;
        let client = k8s_inst.get_client();

        let db = k8s_inst.create_database("simple").await;

        let odcs_db = OpenDcsTestDatabase::new(
            client.clone(),
            "testdb",
            &db,
            "ghcr.io/opendcs/compdepends:main-nightly",
        )
        .await;
        // we don't do anything with the database itself, just make sure it can start and be deleted.
        assert!(odcs_db.delete().await);

        info!("OpenDCS Database Removed.");
        db.close().await.expect("Unable to cleanup resources.");
    }

    #[rstest]
    #[tokio::test]
    async fn test_schema_upgrade(#[future] k8s_inst: &K8s) {
        let k8s_inst = k8s_inst.await;
        let client = k8s_inst.get_client();
        let base_image = "ghcr.io/opendcs/compdepends:main-nightly";
        let upgrade_image = "ghcr.io/opendcs/compdepends:sha-a50092b";

        let db = k8s_inst.create_database("upgrade").await;

        let odcs_db =
            OpenDcsTestDatabase::new(client.clone(), "testdb-upgrade", &db, base_image).await;
        let status = odcs_db.opendcs_database.status.expect("No status?");
        assert!(status.state == Some(MigrationState::Ready));
        assert!(status.applied_schema_version == Some(base_image.into()));

        // start a depending application
        // Change the schema image to trigger migration and wait.
        let odcs_db =
            OpenDcsTestDatabase::new(client.clone(), "testdb-upgrade", &db, upgrade_image).await;

        let status = odcs_db.opendcs_database.status.clone().expect("No status?");
        assert!(status.state == Some(MigrationState::Ready));
        assert!(status.applied_schema_version == Some(upgrade_image.into()));

        assert!(odcs_db.delete().await);

        info!("OpenDCS Database Removed.");
        db.close().await.expect("Unable to cleanup resources.");
    }
}
