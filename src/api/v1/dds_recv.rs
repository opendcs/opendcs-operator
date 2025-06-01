use std::fmt::Debug;

use garde::Validate;
use kube::CustomResource;
use schemars::visit::{visit_schema_object, Visitor};
use schemars::{schema::SchemaObject, JsonSchema};
use serde::{Deserialize, Serialize};

pub struct MyVisitor;

impl Visitor for MyVisitor {
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        schema
            .extensions
            .insert("test".to_string(), serde_json::json!("a test"));
        visit_schema_object(self, schema);
    }
}

// Our custom resource
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, Validate)]
#[kube(
    group = "lrgs.opendcs.org",
    version = "v1",
    kind = "DdsConnection",
    namespaced
)]
#[serde(rename_all = "camelCase")]
#[schemars(schema_with = "add_one_of")]
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

#[allow(unused)]
fn add_one_of(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    let mut schema = String::json_schema(gen);

    // doesn't seem to provide the control desired.

    schema
}

fn port_default() -> i32 {
    16003
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub  enum TlsMode {
    NoTls,
    StartTls,
    Tls,
}