use crate::core::persistence::metrics::model::{MetricLabels, ResourceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Metric semantics are required for **correct rollup and cost calculation**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSemantics {
    /// Instantaneous value (memory usage, fs used bytes)
    Gauge,
    /// Monotonically increasing counter (cpu usage ns, network bytes)
    Counter,
}

/// Minimal but explicit unit definition.
///
/// Avoid over-modeling; this is enough for rollup and cost math.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricUnit {
    Bytes,
    NanoSeconds,
    MilliCores,
    Cores,
    Count,
    Unknown,
}

/// Typed metric value with semantics and unit.
///
/// i128 is used intentionally:
/// - safe for counter delta math
/// - tolerant to reset/negative intermediate values
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricValue {
    pub semantics: MetricSemantics,
    pub unit: MetricUnit,
    pub value: i128,
}

impl MetricValue {
    pub fn gauge(unit: MetricUnit, value: impl Into<i128>) -> Self {
        Self {
            semantics: MetricSemantics::Gauge,
            unit,
            value: value.into(),
        }
    }

    pub fn counter(unit: MetricUnit, value: impl Into<i128>) -> Self {
        Self {
            semantics: MetricSemantics::Counter,
            unit,
            value: value.into(),
        }
    }
}

/// Core metrics commonly required for Kubernetes/Docker cost allocation.
///
/// These fields are intentionally **optional**:
/// - availability differs by platform and resource level
/// - missing data should not break the pipeline
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricsCore {
    // ---- CPU ----
    /// usageCoreNanoSeconds (counter)
    pub cpu_usage_core_nano_seconds: Option<u64>,
    /// derived rate (millicores)
    pub cpu_usage_millicores: Option<u64>,

    // ---- Memory ----
    pub memory_working_set_bytes: Option<u64>,
    pub memory_rss_bytes: Option<u64>,
    pub memory_usage_bytes: Option<u64>,

    // ---- Network ----
    pub network_rx_bytes_total: Option<u64>,
    pub network_tx_bytes_total: Option<u64>,

    // ---- Filesystem ----
    pub fs_used_bytes: Option<u64>,
    pub fs_available_bytes: Option<u64>,
    pub fs_capacity_bytes: Option<u64>,

    // ---- Requests / Limits (for cost allocation) ----
    pub cpu_request_millicores: Option<u64>,
    pub cpu_limit_millicores: Option<u64>,
    pub memory_request_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,

    /// Platform-specific or experimental metrics.
    ///
    /// Examples:
    /// - gpu_utilization
    /// - accelerator_memory_bytes
    /// - tcp_retransmits_total
    pub extras: BTreeMap<String, MetricValue>,
}

/// Canonical metric record used throughout the system.
///
/// This is the **single source of truth** for:
/// - storage
/// - rollup
/// - cost / allocation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformMetric {
    pub timestamp: DateTime<Utc>,

    /// Attribution target (partition key)
    pub resource: ResourceId,

    /// Dimensions for grouping / cost allocation
    #[serde(default)]
    pub labels: MetricLabels,

    #[serde(default)]
    pub metrics: MetricsCore,
}

impl PlatformMetric {
    pub fn new(timestamp: DateTime<Utc>, resource: ResourceId) -> Self {
        Self {
            timestamp,
            resource,
            labels: MetricLabels::default(),
            metrics: MetricsCore::default(),
        }
    }

    pub fn with_label(mut self, k: impl AsRef<str>, v: impl Into<String>) -> Self {
        self.labels.insert(k.as_ref(), v);
        self
    }

    pub fn with_extra(mut self, key: impl Into<String>, value: MetricValue) -> Self {
        self.metrics.extras.insert(key.into(), value);
        self
    }
}