use std::{collections::BTreeMap, fmt::Debug};

use chrono::{DateTime, Utc};
use garde::Validate;
use kube::{CustomResource, KubeSchema, runtime::wait::Condition};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::v1::tsdb::database::MigrationState;

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, KubeSchema, Validate)]
#[kube(
    group = "tsdb.opendcs.org",
    version = "v1",
    kind = "OpenDcsApp",
    status = "OpenDcsAppStatus",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct OpenDcsAppSpec {
    /// Migration image to use. Migration image tags will track the schema version they are as well if the opendcs release version
    #[garde(skip)]
    pub version: String,
    /// Which OpenDcsDatabase to target
    #[garde(skip)]
    pub database: String,
    #[garde(skip)]
    /// Flyway placeholders for the given database. Cannot be changed after initial setup
    #[x_kube(validation = Rule::new("self == oldSelf").message("is immutable"))]
    pub placeholders: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct OpenDcsAppStatus {
    /// Applied Schema version as derived from the installed schema
    pub applied_schema_version: Option<String>,
    /// Current migration activity
    pub state: Option<MigrationState>,
    pub last_updated: Option<DateTime<Utc>>,
}
