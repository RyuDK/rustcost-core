//! Platform-agnostic metric domain model.
//!
//! This module defines the canonical metric representation used across
//! collectors (kubelet, docker, etc), storage, rollup, and cost allocation.
pub mod identity;
pub mod labels;
pub mod metric;

pub use identity::*;
pub use labels::*;
pub use metric::*;