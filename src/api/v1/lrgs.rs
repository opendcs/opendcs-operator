use std::fmt::Debug;

use chrono::{DateTime, Utc};
use garde::Validate;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, Validate)]
#[kube(
    group = "lrgs.opendcs.org",
    version = "v1",
    kind = "LrgsCluster",
    status = "LrgsClusterStatus",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct LrgsClusterSpec {
    #[garde(range(min = 0))]
    pub replicas: i32,
    #[garde(skip)]
    pub storage_class: String,
    #[garde(skip)]
    pub storage_size: String,
    #[garde(range(min = 0))]
    pub archive_length_days: Option<i32>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub  struct LrgsClusterStatus {
    pub checksum: String,
    pub last_updated: Option<DateTime<Utc>>,
}
