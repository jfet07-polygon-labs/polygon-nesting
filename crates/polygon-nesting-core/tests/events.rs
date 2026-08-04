use polygon_nesting_core::{EngineEventSink, EventSequencer};
use polygon_nesting_protocol::{
    EngineEvent, PortfolioPhase, PortfolioProgress, SequencedEngineEvent, StateSnapshot,
};

#[derive(Default)]
struct RecordingSink {
    events: Vec<SequencedEngineEvent>,
}

impl EngineEventSink for RecordingSink {
    fn emit(&mut self, event: SequencedEngineEvent) {
        self.events.push(event);
    }
}

#[test]
fn sequencer_records_semantic_events_in_callback_order() {
    let mut sink = RecordingSink::default();
    let mut sequencer = EventSequencer::new(&mut sink);
    let progress_before = EngineEvent::PortfolioProgress {
        progress: PortfolioProgress {
            phase: PortfolioPhase::SharedArchive,
            best_score: None,
            elapsed_ms: 1.0,
        },
    };
    let snapshot = EngineEvent::StateSnapshot {
        snapshot: StateSnapshot {
            step_index: 2.0,
            beam_rank: 0.0,
            candidate_count: 0.0,
            source: None,
            placements: Vec::new(),
            remaining_prepared_pieces: Vec::new(),
            unplaced_piece_ids: Vec::new(),
        },
        beam_width: 4.0,
    };
    let progress_after = EngineEvent::PortfolioProgress {
        progress: PortfolioProgress {
            phase: PortfolioPhase::Completed,
            best_score: None,
            elapsed_ms: 3.0,
        },
    };

    sequencer.emit(progress_before.clone());
    sequencer.emit(snapshot.clone());
    sequencer.emit(progress_after.clone());
    assert_eq!(sequencer.next_ordinal(), 3);
    drop(sequencer);

    assert_eq!(
        sink.events,
        vec![
            SequencedEngineEvent {
                ordinal: 0,
                event: progress_before,
            },
            SequencedEngineEvent {
                ordinal: 1,
                event: snapshot,
            },
            SequencedEngineEvent {
                ordinal: 2,
                event: progress_after,
            },
        ]
    );
    assert_eq!(sink.events.len(), 3);
}
