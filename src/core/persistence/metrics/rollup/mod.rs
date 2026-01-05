//! Rollup engine.
//!
//! Converts a stream of `PlatformMetric` samples from a source resolution
//! into a target resolution using declarative policies.
pub mod engine;

pub use engine::*;
