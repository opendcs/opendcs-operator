use std::fmt::Debug;

use garde::Validate;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Our custom resource
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, Validate)]
#[kube(
    group = "lrgs.opendcs.org",
    version = "v1",
    kind = "DdsConnection",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct DdsConnectionSpec {
    #[garde(ascii, length(min = 1))]
    pub hostname: String,
    #[serde(default = "port_default")]
    #[garde(range(min = 1, max = 65535))]
    pub port: i32,
    #[garde(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[garde(ascii, length(min = 1))]
    pub username: String,
    #[garde(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_mode: Option<TlsMode>,
}

fn port_default() -> i32 {
    16003
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub enum TlsMode {
    NoTls,
    StartTls,
    Tls,
}
