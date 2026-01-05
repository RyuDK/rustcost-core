use serde::{Deserialize, Serialize};

/// Origin platform of the metric sample.
///
/// This is intentionally coarse-grained.
/// Platform-specific details should live in labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Kubernetes,
    Docker,
    DockerCompose,
    Ecs,
    Nomad,
    Vm,
    Unknown,
}

/// Resource abstraction across platforms.
///
/// Kubernetes:
/// - Node        -> Host
/// - Pod         -> Workload
/// - Container   -> Container
///
/// Docker / VM:
/// - Host        -> Host
/// - Container   -> Container
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Host,
    Workload,
    Container,
    Volume,
    Network,
    Unknown,
}

/// Canonical identifier for metric attribution.
///
/// This is the **primary partition key** for storage and rollup.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId {
    pub platform: Platform,
    pub kind: ResourceKind,

    /// Stable string key.
    ///
    /// Examples:
    /// - k8s node:        "node/ip-10-0-0-1"
    /// - k8s pod:         "pod/nsA/pod-123"
    /// - k8s container:  "container/nsA/pod-123/app"
    /// - docker:         "container/redis"
    pub key: String,
}

impl ResourceId {
    pub fn new(
        platform: Platform,
        kind: ResourceKind,
        key: impl Into<String>,
    ) -> Self {
        Self {
            platform,
            kind,
            key: key.into(),
        }
    }
}
