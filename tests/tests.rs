mod common;

#[cfg(test)]
mod tests {

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
}
