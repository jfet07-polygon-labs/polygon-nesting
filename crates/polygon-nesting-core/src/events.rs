//! Typed job event delivery.
//!
//! The sequencer assigns zero-based, strictly increasing ordinals before
//! forwarding protocol events to the caller-provided sink.

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

    pub fn emit(&mut self, event: EngineEvent) {
        let ordinal = self.next_ordinal;
        self.next_ordinal = ordinal.checked_add(1).expect("event ordinal overflow");
        self.sink.emit(SequencedEngineEvent { ordinal, event });
    }

    pub fn next_ordinal(&self) -> u64 {
        self.next_ordinal
    }

    pub fn sink(&mut self) -> &mut dyn EngineEventSink {
        self.sink
    }
}
