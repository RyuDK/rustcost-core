use serde::{Deserialize, Serialize};

/// Field semantics for correct aggregation.
/// rollup requires distinguishing counters vs gauges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSemantics {
    /// Instantaneous point-in-time reading (memory working set, fs used)
    Gauge,
    /// Monotonic counter (cpu usage ns, network bytes total)
    Counter,
}

/// Rollup strategy used to aggregate samples within a target bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollupFn {
    /// Average of gauge samples in the bucket.
    Avg,
    /// Max of gauge samples in the bucket.
    Max,
    /// Min of gauge samples in the bucket.
    Min,
    /// Sum of values (rare for gauges; useful for pre-aggregated metrics).
    Sum,
    /// Counter delta within the bucket (last - first, reset-aware).
    Delta,
    /// Last observed value in the bucket (useful for some stateful gauges).
    Last,
}

/// Policy for handling counter resets and gaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterResetPolicy {
    /// If counter decreases, treat it as reset and ignore negative delta (clamp to 0).
    ClampToZero,
    /// If counter decreases, treat as reset and use `last` as delta (approximation).
    UseLastAsDelta,
}

/// Specifies how a single field should be rolled up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldSpec {
    pub field: &'static str,
    pub semantics: FieldSemantics,
    pub rollup: RollupFn,

    /// Only applies to Counter + Delta rollups.
    pub counter_reset_policy: CounterResetPolicy,
}

impl FieldSpec {
    pub const fn gauge(field: &'static str, rollup: RollupFn) -> Self {
        Self {
            field,
            semantics: FieldSemantics::Gauge,
            rollup,
            counter_reset_policy: CounterResetPolicy::ClampToZero,
        }
    }

    pub const fn counter_delta(field: &'static str, reset: CounterResetPolicy) -> Self {
        Self {
            field,
            semantics: FieldSemantics::Counter,
            rollup: RollupFn::Delta,
            counter_reset_policy: reset,
        }
    }
}

/// A set of field specifications used by the rollup engine.
///
/// This is intentionally a slice of `FieldSpec` rather than a complex registry.
/// - open-source friendly
/// - simple to extend
#[derive(Debug, Clone)]
pub struct AggregationPolicy {
    pub specs: &'static [FieldSpec],
}

impl AggregationPolicy {
    pub const fn new(specs: &'static [FieldSpec]) -> Self {
        Self { specs }
    }
}

/// Canonical policy for core fields in `MetricsCore`.
///
/// - CPU usage ns: Counter -> Delta per bucket
/// - Network bytes: Counter -> Delta per bucket
/// - Memory working set: Gauge -> Avg (or Max depending on preference)
/// - FS used bytes: Gauge -> Avg
///
/// You can adjust these strategies depending on product needs.
pub mod core_policy {
    use super::*;

    // Note:
    // We use field names matching `MetricsCore` fields.
    // The rollup engine will access these fields by name through a mapping layer
    // (to avoid reflection, we typically implement a "field getter/setter" in engine).
    pub const CORE_SPECS: &[FieldSpec] = &[
        // CPU counters
        FieldSpec::counter_delta("cpu_usage_core_nano_seconds", CounterResetPolicy::ClampToZero),

        // Optional derived CPU rate (if present as gauge)
        FieldSpec::gauge("cpu_usage_millicores", RollupFn::Avg),

        // Memory gauges
        FieldSpec::gauge("memory_working_set_bytes", RollupFn::Avg),
        FieldSpec::gauge("memory_rss_bytes", RollupFn::Avg),
        FieldSpec::gauge("memory_usage_bytes", RollupFn::Avg),

        // Network counters
        FieldSpec::counter_delta("network_rx_bytes_total", CounterResetPolicy::ClampToZero),
        FieldSpec::counter_delta("network_tx_bytes_total", CounterResetPolicy::ClampToZero),

        // FS gauges
        FieldSpec::gauge("fs_used_bytes", RollupFn::Avg),
        FieldSpec::gauge("fs_available_bytes", RollupFn::Avg),
        FieldSpec::gauge("fs_capacity_bytes", RollupFn::Last),

        // Requests/limits are typically step functions -> Last is stable.
        FieldSpec::gauge("cpu_request_millicores", RollupFn::Last),
        FieldSpec::gauge("cpu_limit_millicores", RollupFn::Last),
        FieldSpec::gauge("memory_request_bytes", RollupFn::Last),
        FieldSpec::gauge("memory_limit_bytes", RollupFn::Last),
    ];

    pub const CORE: AggregationPolicy = AggregationPolicy::new(CORE_SPECS);
}
