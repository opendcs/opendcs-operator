use std::{collections::BTreeMap, fmt::Debug};

use chrono::{DateTime, Utc};
use garde::Validate;
use kube::{CustomResource, KubeSchema, runtime::wait::Condition};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, KubeSchema, Validate)]
#[kube(
    group = "tsdb.opendcs.org",
    version = "v1",
    kind = "OpenDcsDatabase",
    status = "OpenDcsDatabaseStatus",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct OpenDcsDatabaseSpec {
    /// Migration image to use. Migration image tags will track the schema version they are as well if the opendcs release version
    #[garde(skip)]
    pub schema_version: String,
    /// Secret for admin user of the database. Must contain the following keys: jdbcUrl, username, password
    #[garde(skip)]
    pub database_secret: String,
    #[garde(skip)]
    /// Flyway placeholders for the given database. Cannot be changed after initial setup
    #[x_kube(validation = Rule::new("self == oldSelf").message("is immutable"))]
    pub placeholders: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct OpenDcsDatabaseStatus {
    /// Applied Schema version as derived from the installed schema
    pub applied_schema_version: Option<String>,
    /// Current migration activity
    pub state: Option<MigrationState>,
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
/// Application Level Database State
pub enum MigrationState {
    /// Schema not yet installed.
    Fresh,
    /// Waiting for apps to shutdown.
    PreparingToMigrate,
    /// Applying Schema updates.
    Migrating,
    /// Apps can start connecting to database again.
    Ready,
    /// Schema migration failed and requires user intervention..
    Failed,
}

impl Condition<OpenDcsDatabase> for OpenDcsDatabase {
    fn matches_object(&self, obj: Option<&OpenDcsDatabase>) -> bool {
        match obj {
            Some(other) => self.metadata.name == other.metadata.name,
            _ => false,
        }
    }
}
