//! Typed core job-service API.
//!
//! Job execution, algorithm dispatch, cache ownership, and diagnostics are
//! implemented by Task 24. This module reserves the typed service boundary
//! without adding execution behavior.

use polygon_nesting_protocol::{EngineError, EngineOutcome, EngineRequest};

use crate::control::CancellationControl;
use crate::events::EngineEventSink;

pub struct Job<'a> {
    request: &'a EngineRequest,
    control: &'a CancellationControl,
    sink: &'a mut dyn EngineEventSink,
}

impl<'a> Job<'a> {
    pub fn new(
        request: &'a EngineRequest,
        control: &'a CancellationControl,
        sink: &'a mut dyn EngineEventSink,
    ) -> Self {
        Self {
            request,
            control,
            sink,
        }
    }

    pub fn run(self) -> Result<EngineOutcome, EngineError> {
        let _ = (self.request, self.control, self.sink);
        todo!("Task 24 job execution behavior")
    }
}

pub fn run(
    request: &EngineRequest,
    control: &CancellationControl,
    sink: &mut dyn EngineEventSink,
) -> Result<EngineOutcome, EngineError> {
    Job::new(request, control, sink).run()
}
