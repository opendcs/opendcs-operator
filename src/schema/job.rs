use std::clone;

use chrono::Utc;
use k8s_openapi::{api::{batch::v1::{Job, JobSpec}, core::v1::{ConfigMap, ConfigMapVolumeSource, Container, EnvVar, Event, PodSpec, PodTemplateSpec, SecurityContext, Volume, VolumeMount}}, apimachinery::pkg::apis::meta::v1::OwnerReference};
use kube::{api::{ObjectMeta, Patch, PatchParams, PostParams}, client, Api, Client, Resource, ResourceExt};
use opendcs_controllers::api::v1::tsdb::database::{MigrationState, OpenDcsDatabase, OpenDcsDatabaseStatus};
use serde_json::json;
use anyhow::Result;
use tracing::info;

use crate::configmap::create_script_config_map;



pub struct MigrationJob {
    database: OpenDcsDatabase,
    owner_ref: OwnerReference,
    job: Option<Job>,
    name: String,
    namespace: String,
    status: Option<OpenDcsDatabaseStatus>,
    state: Option<MigrationState>,
    client: Client
}

impl MigrationJob {
    pub async fn from(database: &OpenDcsDatabase, client: &Client) -> MigrationJob {
        MigrationJob {
            client: client.clone(),
            database: database.clone(),
            owner_ref: database.controller_owner_ref(&()).unwrap(),
            job: None,
            name: database.name_any().clone(),
            namespace: database.namespace().unwrap_or("default".to_string()),
            status: database.status.clone(),
            state: database.status.as_ref().and_then(|s| s.state.clone()),
        }
    }

    pub async fn reconcile(&self) -> Result<(Option<MigrationState>,MigrationState)> {
        match self.status.as_ref() {
        Some(_) => self.check_job().await,
        None => self.create_job().await,
        }
    }


    pub async fn create_job(&self) -> Result<(Option<MigrationState>,MigrationState)> {
        
        info!("Creating schema migration job for {}/{}", &self.namespace, &self.name);
        let old_state = self.state.clone();
        let mut env: Vec<EnvVar> = Vec::new();
        self.database.spec.placeholders.iter().for_each(|(k,v)| {
            env.push(EnvVar { name:k.clone(), value: Some(v.clone()), value_from: None });
        });
        let job = Job {
            metadata: ObjectMeta { 
                name: Some(format!("{}-database-migration", &self.name)),
                namespace: Some(self.namespace.clone()),
                owner_references: Some(vec![self.owner_ref.clone()]),
                ..Default::default()
            },
            spec: Some(JobSpec{
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta { 
                name: Some(format!("{}-database-migration", &self.name)),
                namespace: Some(self.namespace.clone()),
                owner_references: Some(vec![self.owner_ref.clone()]),
                ..Default::default()
            }),
            spec: Some(PodSpec {
                containers: vec![
                    Container {
                        name: "schema-migration".to_string(),
                        image: Some(self.database.spec.schema_version.clone()),
                        command: Some(vec![
                            "/bin/bash".to_string(),
                            "/scripts/schema.sh".to_string()
                        ]),
                        security_context: Some(SecurityContext {
                            allow_privilege_escalation: Some(false),
                            ..Default::default()
                        }),
                        env: Some(env),
                        volume_mounts: Some(vec![
                                VolumeMount {
                                name: "schema-scripts".to_string(),
                                mount_path: "/scripts".to_string(),
                                ..Default::default()
                            },
                        ]),
                    ..Default::default()
                    }
                ],
                volumes: Some(vec![
                    Volume {
                        name: "schema-scripts".to_string(),
                        config_map: Some(ConfigMapVolumeSource {
                            name: format!("{}-schema-scripts", self.owner_ref.name),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }]),
                restart_policy: Some("Never".to_string()),
                ..Default::default()
            })
                },
                ..Default::default()
            }), 
            status: None };
        let patch_name = "database-controller";
        let pp = PatchParams::apply(patch_name);
        let schema_config_map = create_script_config_map(self.namespace.clone(), &self.owner_ref);
        let config_map_api: Api<ConfigMap> = Api::namespaced(self.client.clone(), &self.namespace);
        config_map_api
        .patch(
            &schema_config_map.name_any(),
            &pp,
            &Patch::Apply(schema_config_map),
        )
        .await?;
        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);
        jobs.patch(&job.name_any(), &pp, &Patch::Apply(job)).await?;
        /*let events: Api<Event> =Api::namespaced(self.client.clone(), &self.namespace);
        events.create( &PostParams {
            dry_run: false,
            ..Default::default()
        }, &Event {
            metadata: ObjectMeta { 
                name: Some("State".to_string()), // TODO: needs randomness must be unique
                namespace: Some(self.namespace.clone()),
                owner_references: Some(vec![self.owner_ref.clone()]),
                ..Default::default()
            },
            action: Some("Migration Job created.".to_string()),
            message: Some("Migration Job created.".to_string()),
            ..Default::default()
        }).await?;*/
        Ok((old_state,MigrationState::Fresh))
    }

    pub async fn check_job(&self) -> Result<(Option<MigrationState>,MigrationState)> {
        
        info!("Checking on schema migration job for {}/{}", &self.namespace, &self.name);
        let old_state= self.state.clone();
        return Ok((old_state,MigrationState::Migrating));
}
}