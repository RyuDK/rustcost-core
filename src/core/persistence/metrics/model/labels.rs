use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Labels are dimensions for grouping / allocation.
/// They are NOT identity (identity is ResourceId).
///
/// We wrap the map to:
/// - make the concept explicit (open-source friendly)
/// - allow future normalization/validation without schema break
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricLabels {
    inner: BTreeMap<String, String>,
}

impl MetricLabels {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert or override.
    /// Use snake_case keys to keep storage + diffs stable.
    pub fn insert(&mut self, key: &str, value: impl Into<String>) {
        self.inner.insert(key.to_string(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.inner.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.inner.remove(key)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Read-only view for group-by and export.
    pub fn as_map(&self) -> &BTreeMap<String, String> {
        &self.inner
    }

    /// Consume into the underlying map (useful for storage encoders).
    pub fn into_map(self) -> BTreeMap<String, String> {
        self.inner
    }
}

/// Well-known label keys.
///
/// Prefer these keys to avoid fragmentation across collectors.
pub mod label_keys {
    // Cluster / platform
    pub const CLUSTER: &str = "cluster";
    pub const PLATFORM: &str = "platform";

    // K8s identity-ish dimensions (still useful as group-by dimensions)
    pub const NODE: &str = "node";
    pub const NAMESPACE: &str = "namespace";
    pub const POD: &str = "pod";
    pub const CONTAINER: &str = "container";

    // Workload attribution
    pub const WORKLOAD_KIND: &str = "workload_kind";
    pub const WORKLOAD_NAME: &str = "workload_name";

    // Topology / cost dimensions
    pub const NODEPOOL: &str = "nodepool";
    pub const INSTANCE_TYPE: &str = "instance_type";
    pub const ZONE: &str = "zone";
    pub const REGION: &str = "region";

    // Chargeback / ownership
    pub const TEAM: &str = "team";
    pub const OWNER: &str = "owner";
    pub const PROJECT: &str = "project";
    pub const COST_CENTER: &str = "cost_center";

    // Docker / Compose
    pub const COMPOSE_PROJECT: &str = "compose_project";
    pub const SERVICE: &str = "service";
}