use chrono::Utc;
use k8s_openapi::{api::{batch::v1::{Job, JobSpec}, core::v1::{Container, Event, PodSpec, PodTemplateSpec, SecurityContext}}, apimachinery::pkg::apis::meta::v1::OwnerReference};
use kube::{api::{ObjectMeta, Patch, PatchParams, PostParams}, Api, Client, ResourceExt};
use opendcs_controllers::api::v1::tsdb::database::{MigrationState, OpenDcsDatabase, OpenDcsDatabaseStatus};
use serde_json::json;
use anyhow::Result;
use tracing::info;


pub async fn create_job(database: &OpenDcsDatabase, namespace: &str, oref: &OwnerReference, client: Client) -> Result<(Option<MigrationState>,MigrationState)> {
    
    let database_name = database.metadata.name.as_ref().expect("OpenDcsDatabase resource does not have a name?");
    info!("Creating schema migration job for {}/{}", namespace, database_name);
    let old_state: Option<MigrationState> = match database.status.as_ref() {
        Some(s) => Some(s.state.clone()),
        None => None,
    };
    let job = Job {
        metadata: ObjectMeta { 
            name: Some(format!("{}-database-migration", &database_name)),
            namespace: Some(namespace.to_string()),
            owner_references: Some(vec![oref.clone()]),
            ..Default::default()
        },
        spec: Some(JobSpec{
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta { 
            name: Some(format!("{}-database-migration", &database_name)),
            namespace: Some(namespace.to_string()),
            owner_references: Some(vec![oref.clone()]),
            ..Default::default()
        }),
          spec: Some(PodSpec {
            containers:vec![
                Container {
                name: "lrgs".to_string(),
                image: Some("ghcr.io/opendcs/lrgs:7.0.15-RC03".to_string()),
                command: Some(vec![
                    "/bin/bash".to_string(),
                    "/scripts/lrgs.sh".into(),
                    "-f".into(),
                    "/config/lrgs.conf".into(),
                ]),
                security_context: Some(SecurityContext {
                    allow_privilege_escalation: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            }
                
            ],
            restart_policy: Some("Never".to_string()),
            ..Default::default()
          })
            },
            ..Default::default()
        }), 
        status: None };
    let patch_name = "database-controller";
    let pp = PatchParams::apply(patch_name);
    let jobs: Api<Job> = Api::namespaced(client.clone(), namespace);
    jobs.patch(&job.name_any(), &pp, &Patch::Apply(job)).await?;
    let events: Api<Event> =Api::namespaced(client.clone(), namespace);
    events.create( &PostParams {
        dry_run: false,
        ..Default::default()
    }, &Event {
        metadata: ObjectMeta { 
            name: Some("State".to_string()), // TODO: needs randomness must be unique
            namespace: Some(namespace.to_string()),
            owner_references: Some(vec![oref.clone()]),
            ..Default::default()
        },
        action: Some("Migration Job created.".to_string()),
        message: Some("Migration Job created.".to_string()),
        ..Default::default()
    }).await?;
    Ok((old_state,MigrationState::Fresh))
}

pub async fn check_job(database: &OpenDcsDatabase, namespace: &str, oref: &OwnerReference, client: Client) -> Result<(Option<MigrationState>,MigrationState)> {
    let database_name = database.metadata.name.as_ref().expect("OpenDcsDatabase resource does not have a name?");
    info!("Checking on schema migration job for {}/{}", namespace, database_name);
    let old_state: Option<MigrationState> = match database.status.as_ref() {
        Some(s) => Some(s.state.clone()),
        None => None,
    };
    return Ok((old_state,MigrationState::Migrating));
}