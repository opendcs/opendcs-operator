use std::fmt::Display;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref LRGS_GROUP: String = "lrgs.opendcs.org".to_string();
    pub static ref TSDB_GROUP: String = "tsdb.opendcs.org".to_string();
}

impl Display for TSDB_GROUP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_str())
    }
}
