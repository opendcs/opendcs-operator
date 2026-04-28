use std::collections::BTreeMap;

use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{Container, EnvVar, PodSpec, PodTemplateSpec},
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::{Client, Resource, ResourceExt, api::ObjectMeta};

use crate::api::v1::tsdb::{app::OpenDcsApp, database::OpenDcsDatabase};

pub async fn from(app: &OpenDcsApp, database: &OpenDcsDatabase, _client: &Client) -> Deployment {
    let owner_ref = database.controller_owner_ref(&()).unwrap();
    let namespace = app.namespace().unwrap_or("default".to_string());
    let process_name = &app.spec.application;
    let app_name = &app
        .spec
        .app_name
        .clone()
        .unwrap_or(process_name.to_string());
    let name = format!("app-{}-{}", process_name, app_name);
    let version = app
        .spec
        .version
        .clone()
        .unwrap_or("main-nightly".to_string());

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), name.clone());

    // todo
    // Lookup database secrets and map into template
    // map user extra config into template

    let env: Vec<EnvVar> = vec![];

    return Deployment {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace.clone()),
            owner_references: Some(vec![owner_ref.clone()]),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: app.spec.replicas.or(Some(1)),
            selector: LabelSelector {
                match_labels: Some(labels.clone()),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    namespace: Some(namespace.clone()),
                    owner_references: Some(vec![owner_ref.clone()]),
                    labels: Some(labels.clone()),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        image: Some(format!(
                            "ghcr.io/opendcs/{}:{}",
                            app.spec.application, version
                        )),

                        env: Some(env),

                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };
}
