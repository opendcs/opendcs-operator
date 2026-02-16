#[cfg(test)]
pub mod tests {
    use std::collections::BTreeMap;

    use anyhow::Result;
    use k8s_openapi::{
        api::{
            apps::v1::{Deployment, DeploymentSpec},
            core::v1::{
                Container, ContainerPort, EnvFromSource, PodSpec, PodTemplateSpec,
                ResourceRequirements, Secret, SecretEnvSource, Service, ServicePort, ServiceSpec,
            },
        },
        apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector},
    };
    use kube::{
        Api, Client, ResourceExt,
        api::{DeleteParams, ListParams, ObjectMeta, PostParams},
        runtime::{
            conditions,
            wait::{Condition, await_condition},
        },
    };
    use opendcs_controllers::api::v1::tsdb::database::{MigrationState, OpenDcsDatabase};
    use tracing::debug;

    #[derive(Clone)]
    pub struct PostgresInstance {
        pub secret_name: String,
        app_name: String,
        client: Client,
    }

    impl PostgresInstance {
        /// Close out instance, deletes all resources.
        /// NOTE: will likely put in async drop if/when that is available. However, it does work
        /// to be required to call this as we can then leave things running if there is a test failure.
        pub async fn close(&self) -> Result<()> {
            let app_name = self.app_name.clone();
            let client = self.client.clone();
            let deployment_api: Api<Deployment> = Api::default_namespaced(client.clone());
            let secret_api: Api<Secret> = Api::default_namespaced(client.clone());
            let service_api: Api<Service> = Api::default_namespaced(client.clone());
            let list_params = ListParams::default().labels(&format!("app=={}", &app_name));
            let delete_params = DeleteParams::default();

            let deployments = deployment_api.list(&list_params).await?;
            for inst in deployments {
                let name = &inst.name_any();
                debug!("Deleting Deployment {name}");
                deployment_api.delete(name, &delete_params).await?;
            }

            let services = service_api.list(&list_params).await?;
            for inst in services {
                let name = &inst.name_any();
                debug!("Deleting Service {name}");
                service_api.delete(name, &delete_params).await?;
            }

            let secrets = secret_api.list(&list_params).await?;
            for inst in secrets {
                let name = &inst.name_any();
                debug!("Deleting Secret {name}");
                secret_api.delete(name, &delete_params).await?;
            }
            debug!("Done removing elements.");
            Ok(())
        }
    }

    pub async fn create_postgres_instance(
        client: Client,
        name: &str,
    ) -> anyhow::Result<PostgresInstance> {
        let deployment_api: Api<Deployment> = Api::default_namespaced(client.clone());
        let secret_api: Api<Secret> = Api::default_namespaced(client.clone());
        let service_api: Api<Service> = Api::default_namespaced(client.clone());
        let app_name = format!("postgres-{name}");

        let inst = PostgresInstance {
            secret_name: format!("pg-{name}-test-secret").into(),
            app_name: app_name.clone(),
            client: client.clone(),
        };
        debug!("Destroying existing instance of database.");
        inst.close().await?; // close

        // secret+configmap
        let config = Secret {
            metadata: ObjectMeta {
                name: Some(format!("pg-{name}-test-config").into()),
                labels: Some(BTreeMap::from([("app".into(), app_name.clone())])),
                ..Default::default()
            },
            string_data: Some(BTreeMap::from([
                ("POSTGRES_DB".into(), "dcs".into()),
                ("POSTGRES_USER".into(), "dcs".into()),
                ("POSTGRES_PASSWORD".into(), "dcs_password".into()),
            ])),
            ..Default::default()
        };
        let credentials = Secret {
            metadata: ObjectMeta {
                name: Some(format!("pg-{name}-test-secret").into()),
                labels: Some(BTreeMap::from([("app".into(), app_name.clone())])),
                ..Default::default()
            },
            string_data: Some(BTreeMap::from([
                ("username".into(), "dcs".into()),
                ("password".into(), "dcs_password".into()),
                (
                    "jdbc-uri".into(),
                    format!("jdbc:postgresql://{app_name}.default.svc:5432/dcs").into(),
                ),
            ])),
            ..Default::default()
        };

        // pvc?
        // deployment
        // service
        let pg_deployment = Deployment {
            metadata: ObjectMeta {
                name: Some(app_name.clone()),
                labels: Some(BTreeMap::from([("app".into(), app_name.clone())])),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(1),
                selector: LabelSelector {
                    match_labels: Some(BTreeMap::from([("app".into(), app_name.clone())])),
                    ..Default::default()
                },
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(BTreeMap::from([("app".into(), app_name.clone())])),
                        ..Default::default()
                    }),
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: "postgres".into(),
                            resources: Some(ResourceRequirements {
                                claims: None,
                                limits: Some(BTreeMap::from([
                                    ("cpu".into(), Quantity("1000m".into())),
                                    ("memory".into(), Quantity("512M".into())),
                                ])),
                                requests: None,
                            }),
                            image: Some("postgres:17".into()),
                            image_pull_policy: Some("IfNotPresent".into()),
                            ports: Some(vec![ContainerPort {
                                container_port: 5432,
                                ..Default::default()
                            }]),
                            env_from: Some(vec![EnvFromSource {
                                secret_ref: Some(SecretEnvSource {
                                    name: format!("pg-{name}-test-config").into(),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }]),
                            // ignore pvc for now
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                },
                ..Default::default()
            }),
            status: None,
        };

        let pg_service = Service {
            metadata: ObjectMeta {
                name: Some(app_name.clone()),
                labels: Some(BTreeMap::from([("app".into(), app_name.clone())])),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                type_: Some("NodePort".into()),
                ports: Some(vec![ServicePort {
                    port: 5432,
                    ..Default::default()
                }]),
                selector: Some(BTreeMap::from([("app".into(), app_name.clone())])),
                ..Default::default()
            }),
            status: None,
        };

        let pp = PostParams::default();
        secret_api.create(&pp, &config).await?;
        secret_api.create(&pp, &credentials).await?;
        deployment_api.create(&pp, &pg_deployment).await?;
        service_api.create(&pp, &pg_service).await?;

        let establish = await_condition(
            deployment_api,
            &app_name,
            conditions::is_deployment_completed(),
        );
        let _ = tokio::time::timeout(std::time::Duration::from_secs(300), establish)
            .await
            .expect("postgres could not start in time");

        Ok(inst)
    }

    /// await an OpenDcsDatabase instance to be ready (MigrationState::Ready)
    pub fn odcs_database_ready() -> impl Condition<OpenDcsDatabase> {
        odcs_database_state(MigrationState::Ready)
    }

    /// await an OpenDcsDatabase instance to at a state
    pub fn odcs_database_state(expected_state: MigrationState) -> impl Condition<OpenDcsDatabase> {
        move |obj: Option<&OpenDcsDatabase>| {
            if let Some(db) = &obj {
                if let Some(status) = &db.status {
                    if let Some(state) = &status.state {
                        return *state == expected_state;
                    }
                }
            }
            false
        }
    }
}
