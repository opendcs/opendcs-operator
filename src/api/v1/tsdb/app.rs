use std::{collections::BTreeMap, fmt::Debug};

use chrono::{DateTime, Utc};
use garde::Validate;
use k8s_openapi::api::core::v1::{EnvVar, ResourceRequirements, Volume, VolumeMount};
use kube::{CustomResource, KubeSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::v1::tsdb::database::MigrationState;

/// OpenDcs Applications are run in deployments. The optional parameters are provided
/// That will allow administrators to adjust settings for their local environments
/// if not provided either defaults configured on the operator will be used
/// or nothing.
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
    /// OpenDCS application. Must match the container name
    #[garde(skip)]
    pub application: String,
    /// OpenDCS Application to use, will default to that specified by the operator
    /// If you need more than one, say compproc, set the additional name here.
    #[garde(skip)]
    pub version: Option<String>,
    /// Which OpenDcsDatabase to target
    #[garde(skip)]
    pub database: String,
    // internal application name
    #[garde(skip)]        
    pub app_name: Option<String>,

    /// Allow setting of specific limits
    /// For most apps the default resources are a limit of CPU 1, memory 300m
    /// This covers the majority of usage. The Web-Api will be provided the same
    /// CPU limit with 2000m of memory.
    #[garde(skip)]  
    pub resources: Option<ResourceRequirements>,

    /// Instances of app. NOTE: At this time only set this above 1 on web-api. value will default to one
    #[garde(skip)]
    #[serde(default)]
    pub replicas: Option<i32>,

    /// Optional additional variables. to set in the application environment
    #[garde(skip)]
    #[serde(default)]
    pub extra_env: Option<Vec<EnvVar>>,

    /// Optional additional variables. to set in the application environment
    #[garde(skip)]
    #[serde(default)]
    pub extra_volume_mounts: Option<Vec<VolumeMount>>,

    /// Optional additional variables. to set in the application environment
    #[garde(skip)]
    #[serde(default)]
    pub extra_volumes: Option<Vec<Volume>>,


    /// Optional additional labels to place on the deployment
    #[garde(skip)]
    #[serde(default)]
    pub labels: Option<BTreeMap<String,String>>
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct OpenDcsAppStatus {
    /// Applied Schema version as derived from the installed schema
    pub applied_schema_version: Option<String>,
    /// Current migration activity
    pub state: Option<MigrationState>,
    pub last_updated: Option<DateTime<Utc>>,
}
