use std::clone;

use chrono::Utc;
use k8s_openapi::{api::{batch::v1::{Job, JobSpec}, core::v1::{ConfigMap, ConfigMapVolumeSource, Container, EnvVar, EnvVarSource, Event, PodSpec, PodTemplateSpec, SecretKeySelector, SecretVolumeSource, SecurityContext, Volume, VolumeMount}}, apimachinery::pkg::apis::meta::v1::OwnerReference};
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
    job_name: String,
    status: Option<OpenDcsDatabaseStatus>,
    state: Option<MigrationState>,
    client: Client
}

impl MigrationJob {
    pub async fn from(database: &OpenDcsDatabase, client: &Client) -> MigrationJob {
        let job_name = format!("{}-database-migration", database.name_any());
        let jobs: Api<Job> = Api::namespaced(client.clone(), &database.namespace().unwrap_or("default".to_string()));


        MigrationJob {
            client: client.clone(),
            database: database.clone(),
            owner_ref: database.controller_owner_ref(&()).unwrap(),
            job: jobs.get_opt(&job_name).await.unwrap_or(None),
            name: database.name_any().clone(),
            namespace: database.namespace().unwrap_or("default".to_string()),
            job_name: job_name.clone(),
            status: database.status.clone(),
            state: database.status.as_ref().and_then(|s| s.state.clone()),
        }
    }

    pub async fn reconcile(&self) -> Result<(Option<MigrationState>,MigrationState)> {
        if self.status.as_ref()
               .is_none_or(|status|
                            status.applied_schema_version.as_deref() != Some(&self.database.spec.schema_version)) {
            self.create_job().await
        } else {
            self.check_job().await
        }
    }


    pub async fn create_job(&self) -> Result<(Option<MigrationState>,MigrationState)> {
        
        info!("Creating schema migration job for {}/{}", &self.namespace, &self.name);
        let old_state = self.state.clone();
        let mut env: Vec<EnvVar> = Vec::new();
        self.database.spec.placeholders.iter().for_each(|(k,v)| {
            info!("Adding {k}={v}");
            env.push(EnvVar { name: format!("placeholder_{}",k), value: Some(v.clone()), value_from: None });
        });
        env.push(EnvVar { 
            name: "DATABASE_URL".to_string(), 
            value_from: Some(EnvVarSource{
                secret_key_ref: Some(SecretKeySelector {
                   key: "jdbcUrl".to_string(),
                   name: self.database.spec.database_secret.clone(),
                   optional: Some(true)
                }),
                ..Default::default()
            }), ..Default::default() });
        let job = Job {
            metadata: ObjectMeta { 
                name: Some(self.job_name.clone()),
                namespace: Some(self.namespace.clone()),
                owner_references: Some(vec![self.owner_ref.clone()]),
                ..Default::default()
            },
            spec: Some(JobSpec{
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta { 
                name: Some(self.job_name.clone()),
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
                            VolumeMount {
                                name: "db-admin".to_string(),
                                mount_path: "/secrets/db-admin".to_string(),
                                ..Default::default()
                            },
                            VolumeMount {
                                name: "db-app".to_string(),
                                mount_path: "/secrets/db-app".to_string(),
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
                    },
                    Volume {
                        name: "db-admin".to_string(),
                        secret: Some(SecretVolumeSource { 
                            secret_name: Some(self.database.spec.database_secret.clone()),
                            optional: Some(false),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    Volume {
                        name: "db-app".to_string(),
                        secret: Some(SecretVolumeSource { 
                            secret_name: Some(format!("{}-app-user",self.owner_ref.name.clone())),
                            optional: Some(false),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }
                    ]),
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

        match &self.job {
            Some(job) => {
                let status = job.status.as_ref().unwrap();
                let ready = status.ready.unwrap_or(0);
                let success = status.succeeded.unwrap_or(0);
                if ready > 0 {
                    Ok((old_state,MigrationState::Migrating))
                } else if success > 0 {
                    Ok((old_state,MigrationState::Ready))
                } else {
                    Ok((old_state,MigrationState::PreparingToMigrate))
                }
            },
            None => Ok((old_state,MigrationState::Fresh))
        }
    }
}