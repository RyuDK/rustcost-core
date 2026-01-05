//! Policy layer (declarative).
//!
//! - `time`: defines resolutions and bucketing rules
//! - `aggregation`: defines per-field rollup strategies
pub mod time;
pub mod aggregation;

pub use time::*;
pub use aggregation::*;
