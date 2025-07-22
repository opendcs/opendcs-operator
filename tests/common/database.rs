#[cfg(test)]
pub mod tests {
    use std::collections::BTreeMap;

    use k8s_openapi::{api::{apps::v1::{Deployment, DeploymentSpec}, core::v1::{Container, ContainerPort, EnvFromSource, PodSpec, PodTemplateSpec, ResourceRequirements, Secret, SecretEnvSource, Service, ServicePort, ServiceSpec}}, apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector}};
    use kube::{api::{ObjectMeta, PatchParams, PostParams}, core::Selector, runtime::{conditions, wait::await_condition}, Api, Client};
    use serde::de::IntoDeserializer;
    

    pub struct PostgresCredentials {
        pub secret_name: String
    }

    pub async fn create_postgres_instance(client: Client) -> anyhow::Result<PostgresCredentials> {
        let deployment_api: Api<Deployment> = Api::default_namespaced(client.clone());
        let secret_api: Api<Secret> = Api::default_namespaced(client.clone());
        let service_api: Api<Service> = Api::default_namespaced(client.clone());
        // secret+configmap
        let config = Secret {
                       
            metadata: ObjectMeta { 
                name: Some("test-config".into()),
                ..Default::default()
                },
            string_data: Some(BTreeMap::from([
                ("POSTGRES_DB".into(),"dcs".into()),
                ("POSTGRES_USER".into(),"dcs".into()),
                ("POSTGRES_PASSWORD".into(),"dcs_password".into()),
            ])),
            ..Default::default()
        };

        let credentials = Secret {
                       
            metadata: ObjectMeta { 
                name: Some("test-secret".into()),
                ..Default::default()
                },
            string_data: Some(BTreeMap::from([
                ("username".into(),"dcs".into()),
                ("password".into(),"dcs_password".into()),
                ("jdbcUrl".into(),"jdbc:postgresql://postgres.default.svc:5432/dcs".into())
            ])),
            ..Default::default()
        };

        // pvc?
        // deployment
        // service
        let pg_deployment = Deployment {
            metadata: ObjectMeta {
                name: Some("postgres".into()),
                ..Default::default()
            },
            spec: Some( DeploymentSpec { 
                replicas: Some(1),
                selector: LabelSelector {
                    match_labels: Some(BTreeMap::from([("app".into(),"postgres".into())])),
                    ..Default::default()
                },
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(BTreeMap::from([("app".into(),"postgres".into())])),
                        ..Default::default()
                    }),
                    spec: Some(
                        PodSpec {
                            containers: vec![
                                Container {
                                    name: "postgres".into(),
                                    resources: Some(ResourceRequirements { 
                                        claims: None,
                                        limits: Some(BTreeMap::from([("cpu".into(),Quantity("1000m".into())),("memory".into(),Quantity("512M".into()))])),
                                        requests: None }),
                                    image: Some("postgres:17".into()),
                                    image_pull_policy: Some("IfNotPresent".into()),
                                    ports: Some(vec![ContainerPort {container_port: 5432, ..Default::default()}]),
                                    env_from: Some(vec![EnvFromSource {secret_ref: Some(SecretEnvSource {
                                        name: "test-config".into(),
                                        ..Default::default()
                                    }), ..Default::default()}]),
                                    // ignore pvc for now
                                    ..Default::default()
                                }
                            ],
                            ..Default::default()
                        }
                    ),
                },
                ..Default::default()
            }),
            status: None,
        };

        let pg_service = Service {
            metadata: ObjectMeta {
                name: Some("postgres".into()),
                labels: Some(BTreeMap::from([("app".into(),"postgres".into())])),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                type_: Some("NodePort".into()),
                ports: Some(vec![ServicePort {port: 5432, ..Default::default()},]),
                selector: Some(BTreeMap::from([("app".into(),"postgres".into())])),
                ..Default::default()
            }),
            status: None,
        };

        let pp = PostParams::default();
        secret_api.create(&pp, &config).await?;
        secret_api.create(&pp, &credentials).await?;
        deployment_api.create(&pp, &pg_deployment).await?;
        service_api.create(&pp, &pg_service).await?;

        let establish = await_condition(deployment_api, "postgres", conditions::is_deployment_completed());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(60), establish).await.expect("postgres could not start in time");

        Ok(PostgresCredentials { secret_name: "test-secret".into() })
    }
}