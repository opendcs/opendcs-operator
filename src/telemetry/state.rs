use chrono::{DateTime, Utc};
use kube::{
    runtime::events::{Recorder, Reporter},
    Client, ResourceExt,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::metrics::Metrics;

// Context for our reconciler
#[derive(Clone)]
#[allow(unused)]
pub struct Context<T: Clone + ResourceExt> {
    /// Kubernetes client
    pub client: Client,
    /// Event recorder
    pub recorder: Recorder,
    /// Diagnostics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prometheus metrics
    pub metrics: Arc<Metrics<T>>,
}

/// State shared between the controller and the web server
#[derive(Clone)]
pub struct State<T: Clone + ResourceExt> {
    /// Diagnostics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prometheus metrics
    pub metrics: Arc<Metrics<T>>,
}

/// State wrapper around the controller outputs for the web server
impl<T: Clone + ResourceExt> State<T> {
    /// Metrics getter
    pub fn metrics(&self) -> String {
        let mut buffer = String::new();
        let registry = &*self.metrics.registry;
        prometheus_client::encoding::text::encode(&mut buffer, registry).unwrap();
        buffer
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    pub async fn to_context(&self, client: Client) -> Arc<Context<T>> {
        Arc::new(Context {
            client: client.clone(),
            recorder: self.diagnostics.read().await.recorder(client),
            metrics: self.metrics.clone(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

impl<T: Clone + ResourceExt> Default for State<T> {
    fn default() -> Self {
        Self {
            diagnostics: Default::default(),
            metrics: Default::default(),
        }
    }
}

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "doc-controller".into(),
        }
    }
}
impl Diagnostics {
    fn recorder(&self, client: Client) -> Recorder {
        Recorder::new(client, self.reporter.clone())
    }
}
