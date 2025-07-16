use std::collections::BTreeMap;

use k8s_openapi::{api::core::v1::ConfigMap, apimachinery::pkg::apis::meta::v1::OwnerReference};
use kube::api::ObjectMeta;
use sha1::Digest;

pub fn create_script_config_map(
    namespace: String,
    owner_ref: &OwnerReference,
) -> ConfigMap {
    let script = String::from_utf8(Vec::from(include_bytes!("schema.sh"))).unwrap_or_default();

    ConfigMap {
        metadata: ObjectMeta {
            name: Some(format!("{}-schema-scripts", owner_ref.name)),
            namespace: Some(namespace),
            owner_references: Some(vec![owner_ref.clone()]),
            labels: Some(BTreeMap::from([(
                "tsdb.opendcs.org/for-database".to_string(),
                owner_ref.name.clone(),
            )])),
            ..Default::default()
        },
        data: Some(BTreeMap::from([("schema.sh".to_string(), script)])),
        ..Default::default()
    }
}
