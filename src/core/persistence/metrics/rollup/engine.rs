use crate::core::persistence::metrics::model::{MetricLabels, MetricsCore, PlatformMetric, ResourceId};
use crate::core::persistence::metrics::policy::{
    AggregationPolicy, CounterResetPolicy, FieldSemantics, RollupFn, RollupWindow,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Rollup entry point.
///
/// Assumptions:
/// - Input samples may contain multiple resources interleaved.
/// - For correctness, we do not require strict ordering, but unordered input
///   increases memory usage because we must buffer per (resource, bucket).
///
/// Output:
/// - One `PlatformMetric` per (resource, bucket).
/// - `labels` are taken from the latest sample in the bucket (Last-wins).
pub fn rollup(
    samples: impl IntoIterator<Item = PlatformMetric>,
    window: RollupWindow,
    policy: &AggregationPolicy,
) -> Result<Vec<PlatformMetric>, RollupError> {
    window.validate().map_err(RollupError::InvalidWindow)?;

    let mut buckets: HashMap<BucketKey, BucketState> = HashMap::new();

    for s in samples {
        let bucket_ts = window.target_bucket_start(s.timestamp);
        let key = BucketKey {
            resource: s.resource.clone(),
            bucket_start: bucket_ts,
        };

        let state = buckets.entry(key).or_insert_with(|| BucketState::new(bucket_ts));
        state.observe(s, policy)?;
    }

    // Materialize output (stable ordering by (resource.key, bucket_start))
    let mut out: Vec<(BucketKey, PlatformMetric)> = buckets
        .into_iter()
        .map(|(k, st)| {
            let m = st.finish(k.resource.clone(), policy);
            (k, m)
        })
        .collect();

    out.sort_by(|(a, _), (b, _)| {
        // Deterministic order for storage & diffs
        a.resource.key
            .cmp(&b.resource.key)
            .then_with(|| a.bucket_start.cmp(&b.bucket_start))
    });

    Ok(out.into_iter().map(|(_, m)| m).collect())
}

/// Possible rollup failures.
#[derive(Debug)]
pub enum RollupError {
    InvalidWindow(&'static str),
    UnknownField(&'static str),
}

/// Internal key: one output per (resource, bucket).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BucketKey {
    resource: ResourceId,
    bucket_start: DateTime<Utc>,
}

/// Aggregation state for one bucket across many fields.
#[derive(Debug)]
struct BucketState {
    bucket_start: DateTime<Utc>,

    /// Latest labels in the bucket (Last-wins).
    labels: MetricLabels,

    /// Latest timestamp observed in this bucket (for label last-wins).
    latest_ts: Option<DateTime<Utc>>,

    /// Field-wise aggregation states.
    fields: HashMap<&'static str, FieldAggState>,
}

impl BucketState {
    fn new(bucket_start: DateTime<Utc>) -> Self {
        Self {
            bucket_start,
            labels: MetricLabels::default(),
            latest_ts: None,
            fields: HashMap::new(),
        }
    }

    fn observe(
        &mut self,
        sample: PlatformMetric,
        policy: &AggregationPolicy,
    ) -> Result<(), RollupError> {
        // Keep the latest labels (last-wins) for reporting and chargeback dimensions.
        if self.latest_ts.map(|t| sample.timestamp >= t).unwrap_or(true) {
            self.labels = sample.labels.clone();
            self.latest_ts = Some(sample.timestamp);
        }

        // Apply spec-driven aggregation over core fields.
        for spec in policy.specs {
            let v = get_core_u64(&sample.metrics, spec.field).ok_or(RollupError::UnknownField(spec.field))?;

            if let Some(v) = v {
                let entry = self
                    .fields
                    .entry(spec.field)
                    .or_insert_with(|| FieldAggState::new(*spec));

                entry.observe(v);
            }
        }

        Ok(())
    }

    fn finish(self, resource: ResourceId, policy: &AggregationPolicy) -> PlatformMetric {
        let mut metrics = MetricsCore::default();

        // Produce aggregated values for all known specs
        for spec in policy.specs {
            let agg = self.fields.get(spec.field);
            if let Some(state) = agg {
                if let Some(v) = state.finalize() {
                    set_core_u64(&mut metrics, spec.field, v);
                }
            }
        }

        // NOTE: extras rollup is intentionally not done here yet.
        // extras are platform-specific; we will roll them up via a separate spec registry later
        // (while keeping this engine stable and contributor-friendly).

        let mut out = PlatformMetric::new(self.bucket_start, resource);
        out.labels = self.labels;
        out.metrics = metrics;
        out
    }
}

/// Per-field aggregation state.
#[derive(Debug, Clone)]
struct FieldAggState {
    spec: crate::core::persistence::metrics::policy::FieldSpec,

    // Gauge states
    gauge_count: u64,
    gauge_sum: u128,
    gauge_min: Option<u64>,
    gauge_max: Option<u64>,
    gauge_last: Option<u64>,

    // Counter states (for delta)
    counter_first: Option<u64>,
    counter_last: Option<u64>,
}

impl FieldAggState {
    fn new(spec: crate::core::persistence::metrics::policy::FieldSpec) -> Self {
        Self {
            spec,
            gauge_count: 0,
            gauge_sum: 0,
            gauge_min: None,
            gauge_max: None,
            gauge_last: None,
            counter_first: None,
            counter_last: None,
        }
    }

    fn observe(&mut self, v: u64) {
        match self.spec.semantics {
            FieldSemantics::Gauge => self.observe_gauge(v),
            FieldSemantics::Counter => self.observe_counter(v),
        }
    }

    fn observe_gauge(&mut self, v: u64) {
        self.gauge_count = self.gauge_count.saturating_add(1);
        self.gauge_sum = self.gauge_sum.saturating_add(v as u128);

        self.gauge_min = Some(self.gauge_min.map(|m| m.min(v)).unwrap_or(v));
        self.gauge_max = Some(self.gauge_max.map(|m| m.max(v)).unwrap_or(v));
        self.gauge_last = Some(v);
    }

    fn observe_counter(&mut self, v: u64) {
        if self.counter_first.is_none() {
            self.counter_first = Some(v);
        }
        self.counter_last = Some(v);
    }

    fn finalize(&self) -> Option<u64> {
        match (self.spec.semantics, self.spec.rollup) {
            (FieldSemantics::Gauge, RollupFn::Avg) => {
                if self.gauge_count == 0 {
                    None
                } else {
                    Some((self.gauge_sum / self.gauge_count as u128) as u64)
                }
            }
            (FieldSemantics::Gauge, RollupFn::Max) => self.gauge_max,
            (FieldSemantics::Gauge, RollupFn::Min) => self.gauge_min,
            (FieldSemantics::Gauge, RollupFn::Sum) => Some(self.gauge_sum.min(u64::MAX as u128) as u64),
            (FieldSemantics::Gauge, RollupFn::Last) => self.gauge_last,

            // Counter must be Delta (per our policy layer). If contributors add others, handle here.
            (FieldSemantics::Counter, RollupFn::Delta) => self.finalize_counter_delta(),

            // Fallbacks (conservative)
            (FieldSemantics::Counter, _) => self.finalize_counter_delta(),
            (_, _) => self.gauge_last,
        }
    }

    fn finalize_counter_delta(&self) -> Option<u64> {
        let (first, last) = (self.counter_first?, self.counter_last?);

        if last >= first {
            Some(last - first)
        } else {
            // Counter reset or wrap
            match self.spec.counter_reset_policy {
                CounterResetPolicy::ClampToZero => Some(0),
                CounterResetPolicy::UseLastAsDelta => Some(last),
            }
        }
    }
}

/// ---- Core field accessors (no reflection; explicit mapping) ----
///
/// We keep this explicit on purpose:
/// - easier to audit for correctness
/// - contributor-friendly
/// - avoids macro/derive complexity

fn get_core_u64(core: &MetricsCore, field: &'static str) -> Option<Option<u64>> {
    match field {
        "cpu_usage_core_nano_seconds" => Some(core.cpu_usage_core_nano_seconds),
        "cpu_usage_millicores" => Some(core.cpu_usage_millicores),

        "memory_working_set_bytes" => Some(core.memory_working_set_bytes),
        "memory_rss_bytes" => Some(core.memory_rss_bytes),
        "memory_usage_bytes" => Some(core.memory_usage_bytes),

        "network_rx_bytes_total" => Some(core.network_rx_bytes_total),
        "network_tx_bytes_total" => Some(core.network_tx_bytes_total),

        "fs_used_bytes" => Some(core.fs_used_bytes),
        "fs_available_bytes" => Some(core.fs_available_bytes),
        "fs_capacity_bytes" => Some(core.fs_capacity_bytes),

        "cpu_request_millicores" => Some(core.cpu_request_millicores),
        "cpu_limit_millicores" => Some(core.cpu_limit_millicores),
        "memory_request_bytes" => Some(core.memory_request_bytes),
        "memory_limit_bytes" => Some(core.memory_limit_bytes),

        _ => None,
    }
}

fn set_core_u64(core: &mut MetricsCore, field: &'static str, value: u64) {
    match field {
        "cpu_usage_core_nano_seconds" => core.cpu_usage_core_nano_seconds = Some(value),
        "cpu_usage_millicores" => core.cpu_usage_millicores = Some(value),

        "memory_working_set_bytes" => core.memory_working_set_bytes = Some(value),
        "memory_rss_bytes" => core.memory_rss_bytes = Some(value),
        "memory_usage_bytes" => core.memory_usage_bytes = Some(value),

        "network_rx_bytes_total" => core.network_rx_bytes_total = Some(value),
        "network_tx_bytes_total" => core.network_tx_bytes_total = Some(value),

        "fs_used_bytes" => core.fs_used_bytes = Some(value),
        "fs_available_bytes" => core.fs_available_bytes = Some(value),
        "fs_capacity_bytes" => core.fs_capacity_bytes = Some(value),

        "cpu_request_millicores" => core.cpu_request_millicores = Some(value),
        "cpu_limit_millicores" => core.cpu_limit_millicores = Some(value),
        "memory_request_bytes" => core.memory_request_bytes = Some(value),
        "memory_limit_bytes" => core.memory_limit_bytes = Some(value),

        _ => {
            // unknown fields are ignored at write-time to avoid engine fragility;
            // policy validation can be added to detect this early.
        }
    }
}
