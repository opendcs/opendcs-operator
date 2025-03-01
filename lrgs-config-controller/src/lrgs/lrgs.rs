
use crate::api::v1::{dds_recv::DdsConnection, drgs::DrgsConnection};
use hickory_resolver::TokioAsyncResolver as Resolver;
use k8s_openapi::api::core::v1::Secret;
//use futures::{StreamExt, TryStreamExt};
use kube::{
    api::{Api, ListParams},
    Client,
};
use simple_xml_builder::XMLElement;
//, PostParams}};

use std::{error::Error, vec};
use std::fs::File;

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

pub async fn create_ddsrecv_conf(client: Client, file: File, lrgs_service_dns: &str) -> Result<(), Box<dyn Error>> {
    let mut ddsrecv_conf = XMLElement::new("ddsrecvconf");
    let mut i: i32 = 0;
    // Read pods in the configured namespace into the typed interface from k8s-openapi
    let connections: Api<DdsConnection> = Api::default_namespaced(client.clone());
    // get other lrgs's
    let resolver = Resolver::tokio_from_system_conf().unwrap();
    let recs_res = resolver.srv_lookup(lrgs_service_dns).await;
    
    let recs = match recs_res {
        Ok(_) => recs_res.unwrap().iter().map(|srv| {srv.target().to_ascii()}).collect(),
        Err(e) => match e.kind() {
            hickory_resolver::error::ResolveErrorKind::NoRecordsFound { query: _, soa: _, negative_ttl: _, response_code: _, trusted: _ } => {
                println!("No LRGSes configured to setup replication.");
                Vec::new()
            },
            _ => return Err(Box::new(e))
        }
    };
    for rec in recs {
        let name = format!("replication-{}", i);
        add_dds_connection(&mut ddsrecv_conf, i, &name, &rec, 16003, "replication", true);
        print!("{rec:?}");
        i = i + 1;
    }

    // NOTE: review error handling more. No connections is reasonable, need
    // to make sure this would always just be empty and figure out some other error conditions.
    for host in connections.list(&ListParams::default()).await? {
        println!("found dds {}", host.spec.hostname);
        add_dds_connection(&mut ddsrecv_conf, i, &host.metadata.name.unwrap(), &host.spec.hostname, host.spec.port, &host.spec.username, host.spec.enabled.unwrap_or(false));
        i = i + 1;
    }
    print!("{}", ddsrecv_conf);
    Ok(ddsrecv_conf.write(file)?)
}

pub async fn create_drgsrecv_conf(client: Client, file: File) -> Result<(), Box<dyn Error>> {
    let mut drgsrecv_conf = XMLElement::new("drgsconf");
    let mut i: i32 = 0;
    let drgs_connections: Api<DrgsConnection> = Api::default_namespaced(client.clone());
    for connection in drgs_connections.list(&ListParams::default()).await? {
        println!(
            "{i}: {connection:?}"
        );
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
    Ok(drgsrecv_conf.write(file)?)
}

pub async fn create_password_file(client: Client, file: File) -> Result<(), Box<dyn Error>> {
    let users: Api<Secret> = Api::default_namespaced(client.clone());
    let params = ListParams::default().fields("type=lrgs.opendcs.org/ddsuser");
    let mut pw_file = password_file::PasswordFile::new(file);
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
    Ok(pw_file.write_file()?)
}
