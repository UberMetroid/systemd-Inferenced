//! inferenced-core: Core hardware discovery, compute plane topology,
//! and resource scheduling primitives for systemd-inferenced.

pub mod arbiter;
pub mod error;
pub mod lease;
pub mod model;
pub mod psi;
pub mod topology;

pub use arbiter::Arbiter;
pub use error::{Error, Result};
pub use lease::{ComputeLease, LeaseId, LeasePriority, LeaseState};
pub use model::{ModelDescriptor, ModelPlacementState, ModelRegistry};
pub use psi::{PressureLevel, PressureMetrics};
pub use topology::{ComputePlane, ComputePlaneKind, HardwareTopology};
