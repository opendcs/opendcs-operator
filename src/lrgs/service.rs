use std::collections::BTreeMap;

use k8s_openapi::{
    api::core::v1::{Service, ServicePort, ServiceSpec},
    apimachinery::pkg::{apis::meta::v1::OwnerReference, util::intstr::IntOrString},
};
use kube::{api::ObjectMeta, runtime::reflector::Lookup, Client};
use opendcs_controllers::api::v1::lrgs::LrgsCluster;

pub fn create_service(
    _client: Client,
    lrgs_cluster: &LrgsCluster,
    owner_ref: &OwnerReference,
) -> Vec<Service> {
    let cluster_name = lrgs_cluster.name().unwrap();
    let ns: Option<String> = lrgs_cluster.metadata.namespace.clone();
    vec![
        Service {
            metadata: ObjectMeta {
                name: Some(format!("{}-lrgs-service", &cluster_name)),
                namespace: ns.clone(),
                owner_references: Some(vec![owner_ref.clone()]),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                type_: Some("ClusterIP".to_string()),
                session_affinity: Some("ClientIP".to_string()),
                ports: Some(vec![ServicePort {
                    name: Some("dds".to_string()),
                    port: 16003,
                    target_port: Some(IntOrString::String("dds".to_string())),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                }]),
                selector: Some(BTreeMap::from([(
                    "app.kubernetes.io/name".to_string(),
                    "lrgs".to_string(),
                )])),
                ..Default::default()
            }),
            ..Default::default()
        },
        Service {
            metadata: ObjectMeta {
                name: Some(format!("{}-lrgs-service-headless", &cluster_name)),
                namespace: ns.clone(),
                owner_references: Some(vec![owner_ref.clone()]),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                type_: Some("ClusterIP".to_string()),
                cluster_ip: Some("None".to_string()),
                session_affinity: Some("ClientIP".to_string()),
                ports: Some(vec![ServicePort {
                    name: Some("dds".to_string()),
                    port: 16003,
                    target_port: Some(IntOrString::String("dds".to_string())),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                }]),
                selector: Some(BTreeMap::from([(
                    "app.kubernetes.io/name".to_string(),
                    "lrgs".to_string(),
                )])),
                ..Default::default()
            }),
            ..Default::default()
        },
    ]
}
