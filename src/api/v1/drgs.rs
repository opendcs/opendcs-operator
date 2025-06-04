use std::fmt::Debug;

use garde::Validate;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// <?xml version="1.0"?>
// <drgsconf>

// 	<!-- Debug level is 0 (fewest messages) ... 3 (most verbose) -->
// 	<debug>3</debug>

// 	<!-- Specify up to 64 DRGS connections here... -->

// 	<connection number="0" host="east-drgs-hostname.mydomain.gov">
// 		<name>DRGS-E</name>
// 		<enabled>true</enabled>
// 		<msgport>17010</msgport>
// 		<evtport>17011</evtport>
// 		<evtenabled>true</evtenabled>
// 		<startpattern>534D0D0A</startpattern>
// 	</connection>

// 	<connection number="1" host="west-drgs-hostname.mydomain.gov">
// 		<name>DRGS-W</name>
// 		<enabled>true</enabled>
// 		<msgport>17010</msgport>
// 		<evtport>17011</evtport>
// 		<evtenabled>true</evtenabled>
// 		<startpattern>534D0D0A</startpattern>
// 	</connection>

// 	<!--
// 		Add other connections here (up to number=63)
// 	-->
// </drgsconf>

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, Validate)]
#[kube(
    group = "lrgs.opendcs.org",
    version = "v1",
    kind = "DrgsConnection",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct DrgsConnectionSpec {
    #[garde(ascii, length(min = 1))]
    pub hostname: String,
    #[serde(default = "evt_port_default")]
    #[garde(range(min = 1, max = 65535))]
    pub event_port: u16,
    #[serde(default = "msg_port_default")]
    #[garde(range(min = 1, max = 65535))]
    pub message_port: u16,
    #[garde(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[garde(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_enabled: Option<bool>,
    #[garde(ascii, length(min = 1))]
    pub start_pattern: String,
}

fn evt_port_default() -> u16 {
    17011
}

fn msg_port_default() -> u16 {
    17010
}
