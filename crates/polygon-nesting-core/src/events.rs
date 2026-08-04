//! Core event delivery API.
//!
//! Event serialization and sequencing behavior are implemented by the Task 24
//! execution service. This module only establishes the typed service boundary.

use polygon_nesting_protocol::{EngineEvent, SequencedEngineEvent};

pub trait EngineEventSink: Send {
    fn emit(&mut self, event: SequencedEngineEvent);
}

pub struct EventSequencer<'a> {
    next_ordinal: u64,
    sink: &'a mut dyn EngineEventSink,
}

impl<'a> EventSequencer<'a> {
    pub fn new(sink: &'a mut dyn EngineEventSink) -> Self {
        Self {
            next_ordinal: 0,
            sink,
        }
    }

    pub fn emit(&mut self, _event: EngineEvent) {
        todo!("Task 24 event sequencing behavior")
    }

    pub fn next_ordinal(&self) -> u64 {
        self.next_ordinal
    }

    pub fn sink(&mut self) -> &mut dyn EngineEventSink {
        self.sink
    }
}
