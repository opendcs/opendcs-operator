
use crate::api::{constants::LRGS_GROUP, v1::{dds_recv::DdsConnection, drgs::DrgsConnection, lrgs::{self, LrgsCluster}}};
//use hickory_resolver::TokioAsyncResolver as Resolver;
use k8s_openapi::{api::core::v1::Secret, apimachinery::pkg::apis::meta::v1::OwnerReference, ByteString};
//use futures::{StreamExt, TryStreamExt};
use kube::{
    api::{Api, ListParams, ObjectMeta}, Client
};
use passwords::PasswordGenerator;
use sha2::{Sha256, Digest};
use simple_xml_builder::XMLElement;
use tracing::{debug, warn};
//, PostParams}};

use std::{collections::BTreeMap, fmt::format, vec};
use anyhow::{anyhow, Result};

use super::password_file;

fn add_dds_connection(conf: &mut XMLElement, i: i32, name: &str, hostname: &str, port: i32, username: &str, enabled: bool) {
    let mut connection = XMLElement::new("connection");
    connection.add_attribute("number", i);
    connection.add_attribute("host", hostname);
    let mut xml_enabled = XMLElement::new("enabled");
    xml_enabled.add_text(enabled.to_string());

    let mut xml_port = XMLElement::new("port");
    xml_port.add_text(port);

    let mut xml_name = XMLElement::new("name");
    xml_name.add_text(name);

    let mut xml_username = XMLElement::new("username");
    xml_username.add_text(username);

    let mut authenticate = XMLElement::new("authenticate");
    authenticate.add_text("true");

    connection.add_child(xml_enabled);
    connection.add_child(xml_port);
    connection.add_child(xml_name);
    connection.add_child(xml_username);
    connection.add_child(authenticate);

    conf.add_child(connection);
}

async fn create_ddsrecv_conf(client: Client, namespace: &str) -> Result<String> {
    let mut ddsrecv_conf = XMLElement::new("ddsrecvconf");
    let mut i: i32 = 0;
    // Read pods in the configured namespace into the typed interface from k8s-openapi
    let connections: Api<DdsConnection> = Api::namespaced(client.clone(), &namespace);

    // NOTE: review error handling more. No connections is reasonable, need
    // to make sure this would always just be empty and figure out some other error conditions.
    for host in connections.list(&ListParams::default()).await? {
        println!("found dds {}", host.spec.hostname);
        add_dds_connection(&mut ddsrecv_conf, i, &host.metadata.name.unwrap(), &host.spec.hostname, host.spec.port, &host.spec.username, host.spec.enabled.unwrap_or(false));
        i = i + 1;
    }
    Ok(ddsrecv_conf.to_string())
}

async fn create_drgsrecv_conf(client: Client, namespace: &str) -> Result<String> {
    let mut drgsrecv_conf = XMLElement::new("drgsconf");
    let mut i: i32 = 0;
    let drgs_connections: Api<DrgsConnection> = Api::namespaced(client.clone(), namespace);
    for connection in drgs_connections.list(&ListParams::default()).await? {
        println!("Adding DRGS Connection {i}: {}", connection.spec.hostname);
        let mut xml_connection = XMLElement::new("connection");
        xml_connection.add_attribute("number", i);
        xml_connection.add_attribute("host", connection.spec.hostname);

        let mut xml_name = XMLElement::new("name");
        xml_name.add_text(connection.metadata.name.unwrap());

        let mut xml_enable = XMLElement::new("enabled");
        xml_enable.add_text(connection.spec.enabled.unwrap_or(true));

        let mut xml_msg_port = XMLElement::new("msgport");
        xml_msg_port.add_text(connection.spec.message_port);

        let mut xml_event_port = XMLElement::new("evtport");
        xml_event_port.add_text(connection.spec.event_port);

        let mut xml_event_port_enabled = XMLElement::new("evtenabled");
        xml_event_port_enabled.add_text(connection.spec.event_enabled.unwrap_or(false));

        let mut xml_start_pattern = XMLElement::new("startpattern");
        xml_start_pattern.add_text(connection.spec.start_pattern);

        xml_connection.add_child(xml_name);
        xml_connection.add_child(xml_enable);
        xml_connection.add_child(xml_msg_port);
        xml_connection.add_child(xml_event_port);
        xml_connection.add_child(xml_event_port_enabled);
        xml_connection.add_child(xml_start_pattern);
        drgsrecv_conf.add_child(xml_connection);
        
        i = i +1;
    }
    Ok(drgsrecv_conf.to_string())
}

async fn create_password_file(client: Client, namespace: &str) -> Result<String> {
    let users: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let params = ListParams::default().fields("type=lrgs.opendcs.org/ddsuser");
    let mut pw_file = password_file::PasswordFile::new();
    for user in users.list(&params).await? {
        let data = user.data;
        if data.is_some() {
            let data = data.unwrap();
            let username = String::from_utf8(data.get("username").unwrap().0.clone())?;
            let password = String::from_utf8(data.get("password").unwrap().0.clone())?;
            let roles = data.get("roles");
            let roles = match roles {
                Some(_) => String::from_utf8(roles.unwrap().0.clone())?
                    .split(",")
                    .map(|i| String::from(i))
                    .collect(),
                None => vec![],
            };
            pw_file.add_user(password_file::DdsUser {
                username,
                password,
                roles,
            });
        }
    }
    Ok(pw_file.to_string())
}

pub struct LrgsConfig {
    pub secret: Secret,
    pub hash: String
}

pub async fn create_lrgs_config(client: Client, cluster: &LrgsCluster, owner_ref: &OwnerReference) -> Result<LrgsConfig> {
    let mut hasher = Sha256::new();
    let namespace = cluster.metadata.namespace.clone().expect("LrgsCluster does not have a namespace set.");

    let password_file = create_password_file(client.clone(), &namespace).await?;
    hasher.update(password_file.as_bytes());

    let dds_config = create_ddsrecv_conf(client.clone(), &namespace).await?;
    hasher.update(dds_config.as_bytes());

    let drgs_config = create_drgsrecv_conf(client.clone(), &namespace).await?;
    hasher.update(drgs_config.as_bytes());

    let config_file_data = Vec::from("
archiveDir: /archive
enableDdsRecv: true
ddsRecvConfig: /tmp/ddsrecv.conf
enableDrgsRecv: false
drgsRecvConfig: ${LRGSHOME}/drgsconf.xml
htmlStatusSeconds: 10
ddsListenPort: 16003
ddsRequireAuth: true
# this prevents the LRGS from failing to respond if no data is available
noTimeout: true
    ".to_string());

    let password_file_data = Vec::from(password_file);
    let dds_config_data = Vec::from(dds_config);
    let drgs_config_data = Vec::from(drgs_config);

    let secret = Secret {
        data: Some(
            BTreeMap::from([
                (".lrgs.passwd".to_string(), ByteString(password_file_data)),
                ("ddsrecv.conf".to_string(), ByteString(dds_config_data)),
                ("drgsconf.xml".to_string(), ByteString(drgs_config_data)),
                ("lrgs.conf".to_string(), ByteString(config_file_data))
            ])
        ),
        metadata: ObjectMeta {
            name: Some(format!("{}-lrgs-configuration",&owner_ref.name)),
            namespace: Some(namespace.clone()),
            owner_references: Some(vec![owner_ref.clone()]),
            annotations: Some(
                BTreeMap::from(
                    [
                        (format!("{}/for-cluster",LRGS_GROUP.as_str()).clone(),cluster.metadata.name.clone().unwrap())
                    ]
                )
            ),
            ..Default::default()
            
        },
        ..Default::default()
    };

    let hash = base16ct::lower::encode_string(&hasher.finalize());
    debug!("Calculated hash is: {hash}");
    return Ok(LrgsConfig {
        secret,
        hash
    })
}


pub async fn create_managed_users(client: Client, lrgs_cluster: &LrgsCluster, owner_ref: &OwnerReference) -> Result<Vec<Secret>> {
    let ns = lrgs_cluster.metadata.namespace.clone().unwrap();
    let cluster_name = lrgs_cluster.metadata.name.clone().unwrap();
    let secrets_api: Api<Secret> = Api::namespaced(client, &ns);
    let required = Vec::from(["lrgsadmin","replication","routing-user"]);
    let mut managed_users = Vec::new();
    for user in required {
        match secrets_api.get_opt(user).await? {
            Some(_) => debug!("User already exists."), // Perhaps we should put a rotation here
            None => {
                let password = PasswordGenerator {
                    length: 64,
                    numbers: true,
                    lowercase_letters: true,
                    uppercase_letters: true,
                    symbols: false,
                    spaces: false,
                    exclude_similar_characters: false,
                    strict: true
                }.generate_one().unwrap();
                let roles = match user {
                    "lrgsadmin" => "dds,lrgsadmin",
                    "replication" => "dds",
                    "routing-user" => "dds",
                    &_ => ""
                };
                managed_users.push(
                    Secret {
                        data: Some(
                            BTreeMap::from([
                                ("username".to_string(), ByteString(Vec::from(user))),
                                ("password".to_string(), ByteString(Vec::from(password))),
                                ("roles".to_string(), ByteString(Vec::from(roles)))
                            ])
                        ),
                        type_: Some("lrgs.opendcs.org/ddsuser".to_string()),
                        metadata: ObjectMeta {
                            name: Some(user.to_string()),
                            namespace: lrgs_cluster.metadata.namespace.clone(),
                            owner_references: Some(vec![owner_ref.clone()]),
                            annotations: Some(
                                BTreeMap::from(
                                    [
                                        (format!("{}/for-cluster",LRGS_GROUP.as_str()).clone(), cluster_name.clone())
                                    ]
                                )
                            ),
                            ..Default::default()
                            
                        },
                        ..Default::default()
                    }                
                );
            },
        };
    }

    
    
    Ok(managed_users)
}