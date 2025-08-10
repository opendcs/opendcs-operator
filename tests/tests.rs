mod common;

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use kube::{
        api::{DeleteParams, ListParams, PostParams},
        runtime::wait::await_condition,
        Api,
    };
    use opendcs_controllers::api::v1::tsdb::database::{OpenDcsDatabase, OpenDcsDatabaseSpec};
    use rstest::rstest;
    use tracing::info;

    use crate::common::{
        database::tests::odcs_database_ready,
        tests::{k8s_inst, K8s},
    };

    #[rstest]
    #[tokio::test]
    async fn test_simple_migration(#[future] k8s_inst: &K8s) {
        let k8s_inst = k8s_inst.await;
        let client = k8s_inst.get_client();

        let pp = PostParams::default();
        let db = k8s_inst.create_database("simple").await;
        let odcs_api: Api<OpenDcsDatabase> = Api::default_namespaced(client.clone());
        let test_db_name = "testdb";
        let odcs_database = OpenDcsDatabase::new(
            test_db_name,
            OpenDcsDatabaseSpec {
                schema_version: "ghcr.io/opendcs/compdepends:main-nightly".into(),
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
            .create(&pp, &odcs_database)
            .await
            .expect("Unable to create OpenDCS Database Instance.");
        info!("waiting for odcs db ready.");
        let establish = await_condition(odcs_api.clone(), test_db_name, odcs_database_ready());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(300), establish)
            .await
            .expect("database not created");

        let result = odcs_api
            .delete(test_db_name, &DeleteParams::default())
            .await;

        assert!(result.is_ok());
        info!("OpenDCS Database Removed.");
        db.close().await.expect("Unable to cleanup resources.");
    }
}
