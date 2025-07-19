use k8s_openapi::{
    api::{
        apps::v1::{StatefulSet, StatefulSetSpec},
        core::v1::{
            ConfigMapVolumeSource, Container, ContainerPort, EnvVar, EnvVarSource,
            ObjectFieldSelector, PersistentVolumeClaim, PersistentVolumeClaimSpec,
            PodSecurityContext, PodSpec, PodTemplateSpec, SecretVolumeSource, SecurityContext,
            Volume, VolumeMount, VolumeResourceRequirements,
        },
    },
    apimachinery::pkg::{
        api::resource::Quantity,
        apis::meta::v1::{LabelSelector, OwnerReference},
    },
};
use kube::{api::ObjectMeta, Resource, ResourceExt};

use std::collections::BTreeMap;

use crate::api::{constants::LRGS_GROUP, v1::lrgs::LrgsCluster};

pub fn create_statefulset(
    lrgs_spec: &LrgsCluster,
    config_hash: String,
    script_hash: String,
) -> StatefulSet {
    let owner_ref = lrgs_spec.controller_owner_ref(&()).unwrap();

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), "lrgs".to_string());

    let mut annotations: BTreeMap<String, String> = BTreeMap::new();
    annotations.insert(
        format!("{}/lrgs-config-hash", LRGS_GROUP.as_str()),
        config_hash,
    );
    annotations.insert(
        format!("{}/lrgs-script-hash", LRGS_GROUP.as_str()),
        script_hash,
    );

    let pod_spec = pod_spec_template(lrgs_spec, &owner_ref, &labels, &annotations);
    let pvct = claim_templates(lrgs_spec, &owner_ref, &labels);

    let the_spec = StatefulSetSpec {
        replicas: Some(lrgs_spec.spec.replicas),
        selector: LabelSelector {
            match_expressions: None,
            match_labels: Some(labels.clone()),
        },
        min_ready_seconds: Some(10),
        ordinals: None,
        persistent_volume_claim_retention_policy: None,
        pod_management_policy: None,
        revision_history_limit: None,
        service_name: Some("lrgs".to_string()),
        template: pod_spec,
        update_strategy: None,
        volume_claim_templates: Some(pvct),
    };

    StatefulSet {
        metadata: ObjectMeta {
            name: Some(format!("{}-lrgs", lrgs_spec.metadata.name.clone().unwrap())),
            namespace: lrgs_spec.namespace().clone(),
            owner_references: Some(vec![owner_ref]),
            labels: Some(labels.clone()),
            annotations: Some(annotations.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(the_spec),
        ..Default::default()
    }
}

fn pod_spec_template(
    _lrgs_spec: &LrgsCluster,
    owner_ref: &OwnerReference,
    labels: &BTreeMap<String, String>,
    annotations: &BTreeMap<String, String>,
) -> PodTemplateSpec {
    PodTemplateSpec {
        metadata: Some(ObjectMeta {
            labels: Some(labels.clone()),
            owner_references: Some(vec![owner_ref.clone()]),
            annotations: Some(annotations.clone()),
            name: None,
            namespace: None,
            ..Default::default()
        }),
        spec: Some(PodSpec {
            containers: vec![Container {
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
                ports: Some(vec![ContainerPort {
                    container_port: 16003,
                    name: Some("dds".to_string()),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                }]),
                env: Some(vec![EnvVar {
                    name: "LRGS_INDEX".to_string(),
                    value_from: Some(EnvVarSource {
                        field_ref: Some(ObjectFieldSelector {
                            field_path: "metadata.labels['apps.kubernetes.io/pod-index']".into(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                volume_mounts: Some(vec![
                    VolumeMount {
                        name: "archive".to_string(),
                        mount_path: "/archive".to_string(),
                        ..Default::default()
                    },
                    VolumeMount {
                        name: "lrgs-scripts".to_string(),
                        mount_path: "/scripts".to_string(),
                        ..Default::default()
                    },
                    VolumeMount {
                        name: "lrgs-config".to_string(),
                        mount_path: "/config".to_string(),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }],
            volumes: Some(vec![
                Volume {
                    name: "lrgs-scripts".to_string(),
                    config_map: Some(ConfigMapVolumeSource {
                        name: format!("{}-lrgs-scripts", owner_ref.name),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Volume {
                    name: "lrgs-config".to_string(),
                    secret: Some(SecretVolumeSource {
                        secret_name: Some(format!("{}-lrgs-configuration", owner_ref.name)),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ]),
            security_context: Some(PodSecurityContext {
                fs_group: Some(1000),
                fs_group_change_policy: Some("OnRootMismatch".into()),
                run_as_group: Some(1000),
                run_as_non_root: Some(true),
                run_as_user: Some(1000),
                ..Default::default()
            }),
            ..Default::default()
        }),
    }
}

fn claim_templates(
    lrgs_spec: &LrgsCluster,
    owner_ref: &OwnerReference,
    _labels: &BTreeMap<String, String>,
) -> Vec<PersistentVolumeClaim> {
    vec![PersistentVolumeClaim {
        metadata: ObjectMeta {
            name: Some("archive".to_string()),
            namespace: lrgs_spec.namespace().clone(),
            owner_references: Some(vec![owner_ref.clone()]),
            ..Default::default()
        },
        spec: Some(PersistentVolumeClaimSpec {
            storage_class_name: Some(lrgs_spec.spec.storage_class.clone()),
            resources: Some(VolumeResourceRequirements {
                limits: None,
                requests: Some(BTreeMap::from([(
                    "storage".to_string(),
                    Quantity(lrgs_spec.spec.storage_size.clone()),
                )])),
            }),
            access_modes: Some(vec!["ReadWriteOnce".to_string()]),
            ..Default::default()
        }),
        status: None,
    }]
}
