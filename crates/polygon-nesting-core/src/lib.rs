pub mod archive;
pub mod caches;
pub mod canonical_grid;
pub mod capacity;
pub mod checkpoints;
pub mod clipper;
mod control;
pub mod domain;
mod events;
pub mod geometry;
mod job;
pub mod js_number;
pub mod nfp_ifp;
pub mod parallel;
pub mod result;
pub mod search;
pub mod short_side;
mod trace;
pub mod transforms;
pub mod validation;

pub use control::{CancelReason, CancellationControl};
pub use events::{EngineEventSink, EventSequencer};
pub use job::{run, Job};

use polygon_nesting_protocol::EngineInfo;

pub fn engine_info() -> EngineInfo {
    EngineInfo {
        name: "polygon-nesting".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}
