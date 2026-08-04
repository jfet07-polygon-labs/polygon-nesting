mod archive;
mod caches;
mod canonical_grid;
mod capacity;
mod checkpoints;
mod clipper;
mod control;
mod domain;
mod events;
mod geometry;
mod job;
mod js_number;
mod nfp_ifp;
mod parallel;
mod result;
mod search;
mod short_side;
mod trace;
mod transforms;
mod validation;

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
