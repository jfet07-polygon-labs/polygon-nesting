//! Ordered N-API event delivery for a typed core job.
//!
//! The core owns semantic event generation and zero-based ordinals. This
//! module forwards those frames unchanged, appends one adapter-owned terminal
//! frame, and keeps callback lifecycle failure separate from core outcomes.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::{self, Thread};

use napi::bindgen_prelude::Unknown;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::Status;
use polygon_nesting_protocol::SequencedEngineEvent;

/// A JavaScript callback accepting one serialized event frame.
pub type JsonEventFn =
    ThreadsafeFunction<String, Unknown<'static>, String, Status, false, false, 0>;

/// A typed frame delivered by the N-API adapter.
#[derive(Debug, Clone, PartialEq)]
pub enum EventFrame {
    /// A semantic frame emitted by the core with its core-owned ordinal.
    Core(SequencedEngineEvent),
    /// The one adapter-owned terminal marker for a job.
    Terminal { ordinal: u64 },
}

impl EventFrame {
    /// Returns the frame ordinal without translating core-owned ordinals.
    pub fn ordinal(&self) -> u64 {
        match self {
            Self::Core(event) => event.ordinal,
            Self::Terminal { ordinal } => *ordinal,
        }
    }

    fn json(&self) -> String {
        match self {
            Self::Core(sequenced_event) => {
                let event = serde_json::to_string(&sequenced_event.event)
                    .expect("protocol event always serializes to JSON");
                let kind_end = event
                    .find(',')
                    .expect("internally tagged protocol event has a payload field");
                format!(
                    "{}\"ordinal\":{},{}",
                    &event[..=kind_end],
                    sequenced_event.ordinal,
                    &event[kind_end + 1..]
                )
            }
            Self::Terminal { ordinal } => {
                serde_json::json!({ "kind": "terminal", "ordinal": ordinal }).to_string()
            }
        }
    }
}

const TERMINAL_PENDING: i32 = -1;
const TERMINAL_ADMITTING: i32 = -2;
const TERMINAL_WAITING: i32 = -3;
const TERMINAL_CLOSING: i32 = -4;
const TERMINAL_CLEANUP_ADMITTING: i32 = -5;

struct TerminalLatchState {
    outcome: AtomicI32,
    cleanup_requested: AtomicBool,
    waiter: OnceLock<Thread>,
}

/// Coordinates terminal callback acknowledgement with environment cleanup.
#[derive(Clone)]
pub struct TerminalLatch {
    state: Arc<TerminalLatchState>,
}

/// Single-use acknowledgement passed to the terminal callback transport.
pub struct TerminalAcknowledgement {
    latch: TerminalLatch,
}

impl TerminalLatch {
    /// Creates a latch in its pending state.
    pub fn new() -> Self {
        Self {
            state: Arc::new(TerminalLatchState {
                outcome: AtomicI32::new(TERMINAL_PENDING),
                cleanup_requested: AtomicBool::new(false),
                waiter: OnceLock::new(),
            }),
        }
    }

    /// Admits the terminal callback then waits for its acknowledgement.
    pub fn enqueue_terminal_and_wait(
        &self,
        enqueue: impl FnOnce(TerminalAcknowledgement) -> Status,
    ) -> Status {
        match self.state.outcome.compare_exchange(
            TERMINAL_PENDING,
            TERMINAL_ADMITTING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {}
            Err(outcome) => return status_for_terminal_outcome(outcome),
        }

        let waiter_registration = self.state.waiter.set(thread::current());
        debug_assert!(waiter_registration.is_ok());
        let enqueue_status = enqueue(TerminalAcknowledgement {
            latch: self.clone(),
        });
        if enqueue_status != Status::Ok {
            self.commit_enqueue_failure(enqueue_status);
            return status_for_terminal_outcome(self.state.outcome.load(Ordering::SeqCst));
        }

        self.complete_successful_admission();
        self.commit_cleanup_if_requested();

        loop {
            let outcome = self.state.outcome.load(Ordering::SeqCst);
            if is_terminal_outcome(outcome) {
                return status_for_terminal_outcome(outcome);
            }
            thread::park();
            self.commit_cleanup_if_requested();
        }
    }

    /// Requests closing during environment cleanup and wakes any waiter.
    pub fn close_for_cleanup(&self) {
        self.state.cleanup_requested.store(true, Ordering::SeqCst);
        self.commit_cleanup_if_requested();
        self.unpark_waiter();
    }

    fn acknowledge(&self, status: Status) {
        self.commit_status(status);
    }

    fn complete_successful_admission(&self) {
        loop {
            let current = self.state.outcome.load(Ordering::SeqCst);
            let next = match current {
                TERMINAL_ADMITTING => TERMINAL_WAITING,
                TERMINAL_CLEANUP_ADMITTING => TERMINAL_CLOSING,
                _ => return,
            };
            if self
                .state
                .outcome
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return;
            }
        }
    }

    fn commit_enqueue_failure(&self, status: Status) {
        let status_code = i32::from(status);
        loop {
            let current = self.state.outcome.load(Ordering::SeqCst);
            if is_terminal_outcome(current) {
                return;
            }
            if self
                .state
                .outcome
                .compare_exchange(current, status_code, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                self.unpark_waiter();
                return;
            }
        }
    }

    fn commit_status(&self, status: Status) {
        let status_code = i32::from(status);
        loop {
            let current = self.state.outcome.load(Ordering::SeqCst);
            if is_terminal_outcome(current) || current == TERMINAL_CLEANUP_ADMITTING {
                return;
            }
            if self
                .state
                .outcome
                .compare_exchange(current, status_code, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                self.unpark_waiter();
                return;
            }
        }
    }

    fn commit_cleanup_if_requested(&self) {
        if !self.state.cleanup_requested.load(Ordering::SeqCst) {
            return;
        }
        loop {
            let current = self.state.outcome.load(Ordering::SeqCst);
            if is_terminal_outcome(current) || current == TERMINAL_CLEANUP_ADMITTING {
                return;
            }
            let next = if current == TERMINAL_ADMITTING {
                TERMINAL_CLEANUP_ADMITTING
            } else {
                TERMINAL_CLOSING
            };
            if self
                .state
                .outcome
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return;
            }
        }
    }

    fn unpark_waiter(&self) {
        if let Some(waiter) = self.state.waiter.get() {
            waiter.unpark();
        }
    }
}

impl Default for TerminalLatch {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalAcknowledgement {
    /// Resolves the terminal callback wait with the callback transport status.
    pub fn acknowledge(self, status: Status) {
        self.latch.acknowledge(status);
    }
}

fn is_terminal_outcome(outcome: i32) -> bool {
    outcome == TERMINAL_CLOSING || outcome >= 0
}

fn status_for_terminal_outcome(outcome: i32) -> Status {
    match outcome {
        TERMINAL_CLOSING => Status::Closing,
        outcome if outcome >= 0 => Status::from(outcome),
        _ => unreachable!("terminal latch outcome must be committed"),
    }
}

#[derive(Default)]
struct DeliveryFailure {
    first: Option<Status>,
}

impl DeliveryFailure {
    fn record(&mut self, status: Status) {
        if self.first.is_none() {
            self.first = Some(status);
        }
    }

    fn status(&self) -> Option<Status> {
        self.first
    }
}

/// Bridges typed core frames to a JavaScript callback transport.
pub struct EventBridge {
    next_ordinal: u64,
    terminal_emitted: bool,
    terminal_latch: TerminalLatch,
    delivery_failure: DeliveryFailure,
}

impl EventBridge {
    /// Creates a bridge that appends its terminal frame through `terminal_latch`.
    pub fn new(terminal_latch: TerminalLatch) -> Self {
        Self {
            next_ordinal: 0,
            terminal_emitted: false,
            terminal_latch,
            delivery_failure: DeliveryFailure::default(),
        }
    }

    /// Delivers one semantic core frame, retaining callback failure separately.
    pub fn deliver_core(
        &mut self,
        event: SequencedEngineEvent,
        deliver: impl FnOnce(EventFrame) -> Status,
    ) -> Status {
        if self.terminal_emitted {
            return Status::Ok;
        }
        let Some(next_ordinal) = event.ordinal.checked_add(1) else {
            self.delivery_failure.record(Status::InvalidArg);
            return Status::InvalidArg;
        };
        if event.ordinal != self.next_ordinal {
            self.delivery_failure.record(Status::InvalidArg);
            return Status::InvalidArg;
        }
        self.next_ordinal = next_ordinal;
        let status = deliver(EventFrame::Core(event));
        if status != Status::Ok {
            self.delivery_failure.record(status);
        }
        status
    }

    /// Appends one terminal frame and waits until its callback acknowledges it.
    pub fn emit_terminal_and_wait(
        &mut self,
        enqueue: impl FnOnce(EventFrame, TerminalAcknowledgement) -> Status,
    ) -> Status {
        if self.terminal_emitted {
            return Status::Ok;
        }
        self.terminal_emitted = true;
        let frame = EventFrame::Terminal {
            ordinal: self.next_ordinal,
        };
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .expect("event ordinal overflow after terminal");
        let status = self
            .terminal_latch
            .enqueue_terminal_and_wait(|acknowledgement| enqueue(frame, acknowledgement));
        if status != Status::Ok {
            self.delivery_failure.record(status);
        }
        status
    }

    /// Returns the first callback transport failure, if one occurred.
    pub fn first_delivery_failure(&self) -> Option<Status> {
        self.delivery_failure.status()
    }

    /// Delivers a core frame through the N-API callback without blocking.
    pub fn deliver_core_to_js(
        &mut self,
        event: SequencedEngineEvent,
        callback: &JsonEventFn,
    ) -> Status {
        self.deliver_core(event, |frame| {
            callback.call(frame.json(), ThreadsafeFunctionCallMode::NonBlocking)
        })
    }

    /// Delivers and acknowledges the terminal frame through the N-API callback.
    pub fn emit_terminal_to_js_and_wait(&mut self, callback: &JsonEventFn) -> Status {
        self.emit_terminal_and_wait(|frame, acknowledgement| {
            callback.call_with_return_value(
                frame.json(),
                ThreadsafeFunctionCallMode::NonBlocking,
                move |callback_result, _env| {
                    let status = callback_result
                        .map(|_| Status::Ok)
                        .unwrap_or_else(|error| error.status);
                    acknowledgement.acknowledge(status);
                    Ok(())
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use napi::Status;
    use polygon_nesting_protocol::{
        EngineEvent, PortfolioPhase, PortfolioProgress, SequencedEngineEvent,
    };

    use super::*;

    fn progress(ordinal: u64, phase: PortfolioPhase) -> SequencedEngineEvent {
        SequencedEngineEvent {
            ordinal,
            event: EngineEvent::PortfolioProgress {
                progress: PortfolioProgress {
                    phase,
                    best_score: None,
                    elapsed_ms: 1.0,
                },
            },
        }
    }

    fn snapshot(ordinal: u64) -> SequencedEngineEvent {
        SequencedEngineEvent {
            ordinal,
            event: EngineEvent::StateSnapshot {
                snapshot: polygon_nesting_protocol::StateSnapshot {
                    step_index: 2.0,
                    beam_rank: 3.0,
                    candidate_count: 4.0,
                    source: Some(polygon_nesting_protocol::StateSnapshotSource::Beam),
                    placements: Vec::new(),
                    remaining_prepared_pieces: Vec::new(),
                    unplaced_piece_ids: Vec::new(),
                },
                beam_width: 1.5,
            },
        }
    }

    #[test]
    fn rejects_gapped_core_ordinal_without_delivery_or_ordinal_mutation() {
        let mut bridge = EventBridge::new(TerminalLatch::new());
        let mut delivery_called = false;
        let mut terminal_frame = None;

        assert_eq!(
            bridge.deliver_core(progress(7, PortfolioPhase::SharedArchive), |_frame| {
                delivery_called = true;
                Status::QueueFull
            }),
            Status::InvalidArg
        );
        assert!(!delivery_called);
        assert_eq!(bridge.first_delivery_failure(), Some(Status::InvalidArg));
        assert_eq!(
            bridge.emit_terminal_and_wait(|frame, acknowledgement| {
                terminal_frame = Some(frame);
                acknowledgement.acknowledge(Status::Ok);
                Status::Ok
            }),
            Status::Ok
        );
        assert_eq!(terminal_frame, Some(EventFrame::Terminal { ordinal: 0 }));
        assert_eq!(bridge.first_delivery_failure(), Some(Status::InvalidArg));
    }

    #[test]
    fn rejects_maximum_core_ordinal_without_delivery_or_panic() {
        let mut bridge = EventBridge {
            next_ordinal: u64::MAX,
            terminal_emitted: false,
            terminal_latch: TerminalLatch::new(),
            delivery_failure: DeliveryFailure::default(),
        };
        let mut delivery_called = false;

        assert_eq!(
            bridge.deliver_core(
                progress(u64::MAX, PortfolioPhase::SharedArchive),
                |_frame| {
                    delivery_called = true;
                    Status::QueueFull
                }
            ),
            Status::InvalidArg
        );
        assert!(!delivery_called);
        assert_eq!(bridge.next_ordinal, u64::MAX);
        assert_eq!(bridge.first_delivery_failure(), Some(Status::InvalidArg));
    }

    #[test]
    fn serializes_progress_and_snapshot_callback_frames() {
        assert_eq!(
            EventFrame::Core(progress(7, PortfolioPhase::SharedArchive)).json(),
            r#"{"kind":"portfolio-progress","ordinal":7,"progress":{"phase":"shared_archive","elapsedMs":1.0}}"#
        );
        assert_eq!(
            EventFrame::Core(snapshot(8)).json(),
            r#"{"kind":"state-snapshot","ordinal":8,"snapshot":{"stepIndex":2.0,"beamRank":3.0,"candidateCount":4.0,"source":"beam","placements":[],"remainingPreparedPieces":[],"unplacedPieceIds":[]},"beamWidth":1.5}"#
        );
    }

    #[test]
    fn forwards_core_events_in_order_and_appends_terminal_after_last_core_ordinal() {
        let mut bridge = EventBridge::new(TerminalLatch::new());
        let mut frames = Vec::new();

        bridge.deliver_core(progress(0, PortfolioPhase::SharedArchive), |frame| {
            frames.push(frame);
            Status::Ok
        });
        bridge.deliver_core(progress(1, PortfolioPhase::Completed), |frame| {
            frames.push(frame);
            Status::Ok
        });
        let status = bridge.emit_terminal_and_wait(|frame, acknowledgement| {
            frames.push(frame);
            acknowledgement.acknowledge(Status::Ok);
            Status::Ok
        });

        assert_eq!(status, Status::Ok);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].ordinal(), 0);
        assert_eq!(frames[1].ordinal(), 1);
        assert_eq!(frames[2], EventFrame::Terminal { ordinal: 2 });
    }

    #[test]
    fn terminal_waits_for_callback_acknowledgement() {
        let latch = TerminalLatch::new();
        let bridge_latch = latch.clone();
        let retained_acknowledgement = Arc::new(Mutex::new(None));
        let retained_for_worker = Arc::clone(&retained_acknowledgement);
        let (admitted_sender, admitted_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();

        let worker = thread::spawn(move || {
            let mut bridge = EventBridge::new(bridge_latch);
            let status = bridge.emit_terminal_and_wait(|_frame, acknowledgement| {
                *retained_for_worker
                    .lock()
                    .expect("retained acknowledgement lock") = Some(acknowledgement);
                admitted_sender
                    .send(())
                    .expect("terminal admission notification");
                Status::Ok
            });
            result_sender
                .send(status)
                .expect("terminal result notification");
        });

        admitted_receiver.recv().expect("terminal admission");
        assert_eq!(result_receiver.try_recv(), Err(mpsc::TryRecvError::Empty));
        retained_acknowledgement
            .lock()
            .expect("retained acknowledgement lock")
            .take()
            .expect("retained acknowledgement")
            .acknowledge(Status::Ok);

        assert_eq!(result_receiver.recv().expect("terminal result"), Status::Ok);
        worker.join().expect("worker completes");
    }

    #[test]
    fn cleanup_hook_wakes_terminal_waiter() {
        let latch = TerminalLatch::new();
        let bridge_latch = latch.clone();
        let (admitted_sender, admitted_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();

        let worker = thread::spawn(move || {
            let mut bridge = EventBridge::new(bridge_latch);
            let status = bridge.emit_terminal_and_wait(|_frame, _acknowledgement| {
                admitted_sender
                    .send(())
                    .expect("terminal admission notification");
                Status::Ok
            });
            result_sender
                .send(status)
                .expect("terminal result notification");
        });

        admitted_receiver.recv().expect("terminal admission");
        latch.close_for_cleanup();

        assert_eq!(
            result_receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("cleanup wakes terminal waiter"),
            Status::Closing
        );
        worker.join().expect("worker completes");
    }

    #[test]
    fn cleanup_before_terminal_admission_prevents_enqueue() {
        let latch = TerminalLatch::new();
        latch.close_for_cleanup();
        let mut bridge = EventBridge::new(latch);
        let mut enqueued = false;

        let status = bridge.emit_terminal_and_wait(|_frame, _acknowledgement| {
            enqueued = true;
            Status::Ok
        });

        assert_eq!(status, Status::Closing);
        assert!(!enqueued);
    }

    #[test]
    fn cleanup_does_not_block_during_terminal_enqueue_admission() {
        let latch = TerminalLatch::new();
        let waiter_latch = latch.clone();
        let (enqueue_entered_sender, enqueue_entered_receiver) = mpsc::channel();
        let (release_enqueue_sender, release_enqueue_receiver) = mpsc::channel();
        let (close_returned_sender, close_returned_receiver) = mpsc::channel();

        let waiter = thread::spawn(move || {
            waiter_latch.enqueue_terminal_and_wait(|_acknowledgement| {
                enqueue_entered_sender
                    .send(())
                    .expect("enqueue entered notification");
                release_enqueue_receiver
                    .recv()
                    .expect("enqueue release notification");
                Status::Ok
            })
        });

        enqueue_entered_receiver
            .recv()
            .expect("terminal enqueue entered");
        let cleanup_latch = latch.clone();
        let cleanup = thread::spawn(move || {
            cleanup_latch.close_for_cleanup();
            close_returned_sender
                .send(())
                .expect("cleanup close returned notification");
        });

        let close_returned = close_returned_receiver.recv_timeout(Duration::from_secs(5));
        release_enqueue_sender
            .send(())
            .expect("release terminal enqueue");
        cleanup.join().expect("cleanup thread completes");
        assert_eq!(
            waiter.join().expect("waiter thread completes"),
            Status::Closing
        );
        assert_eq!(close_returned, Ok(()));
    }

    #[test]
    fn callback_failure_wins_over_later_cleanup() {
        let latch = TerminalLatch::new();
        let worker_latch = latch.clone();
        let retained_acknowledgement = Arc::new(Mutex::new(None));
        let retained_for_worker = Arc::clone(&retained_acknowledgement);
        let (admitted_sender, admitted_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();

        let worker = thread::spawn(move || {
            let mut bridge = EventBridge::new(worker_latch);
            let status = bridge.emit_terminal_and_wait(|_frame, acknowledgement| {
                *retained_for_worker
                    .lock()
                    .expect("retained acknowledgement lock") = Some(acknowledgement);
                admitted_sender
                    .send(())
                    .expect("terminal admission notification");
                Status::Ok
            });
            result_sender
                .send(status)
                .expect("terminal result notification");
        });

        admitted_receiver.recv().expect("terminal admission");
        retained_acknowledgement
            .lock()
            .expect("retained acknowledgement lock")
            .take()
            .expect("retained acknowledgement")
            .acknowledge(Status::PendingException);
        latch.close_for_cleanup();

        assert_eq!(
            result_receiver
                .recv()
                .expect("callback failure releases waiter"),
            Status::PendingException
        );
        worker.join().expect("worker completes");
    }

    #[test]
    fn cleanup_wins_over_late_callback_acknowledgement() {
        let latch = TerminalLatch::new();
        let worker_latch = latch.clone();
        let retained_acknowledgement = Arc::new(Mutex::new(None));
        let retained_for_worker = Arc::clone(&retained_acknowledgement);
        let (admitted_sender, admitted_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();

        let worker = thread::spawn(move || {
            let mut bridge = EventBridge::new(worker_latch);
            let status = bridge.emit_terminal_and_wait(|_frame, acknowledgement| {
                *retained_for_worker
                    .lock()
                    .expect("retained acknowledgement lock") = Some(acknowledgement);
                admitted_sender
                    .send(())
                    .expect("terminal admission notification");
                Status::Ok
            });
            result_sender
                .send(status)
                .expect("terminal result notification");
        });

        admitted_receiver.recv().expect("terminal admission");
        latch.close_for_cleanup();
        retained_acknowledgement
            .lock()
            .expect("retained acknowledgement lock")
            .take()
            .expect("retained acknowledgement")
            .acknowledge(Status::Ok);

        assert_eq!(
            result_receiver.recv().expect("cleanup releases waiter"),
            Status::Closing
        );
        worker.join().expect("worker completes");
    }

    #[test]
    fn repeated_terminal_emission_is_suppressed() {
        let mut bridge = EventBridge::new(TerminalLatch::new());
        let mut repeated_enqueue_called = false;

        assert_eq!(
            bridge.emit_terminal_and_wait(|_frame, acknowledgement| {
                acknowledgement.acknowledge(Status::Ok);
                Status::Ok
            }),
            Status::Ok
        );
        assert_eq!(
            bridge.emit_terminal_and_wait(|_frame, _acknowledgement| {
                repeated_enqueue_called = true;
                Status::Ok
            }),
            Status::Ok
        );
        assert!(!repeated_enqueue_called);
    }

    #[test]
    fn core_event_after_terminal_is_suppressed_without_delivery_or_failure() {
        let mut bridge = EventBridge::new(TerminalLatch::new());
        let mut delivery_called = false;

        bridge.deliver_core(progress(0, PortfolioPhase::SharedArchive), |_frame| {
            Status::Ok
        });
        assert_eq!(
            bridge.emit_terminal_and_wait(|_frame, acknowledgement| {
                acknowledgement.acknowledge(Status::Ok);
                Status::Ok
            }),
            Status::Ok
        );
        assert_eq!(
            bridge.deliver_core(progress(2, PortfolioPhase::Completed), |_frame| {
                delivery_called = true;
                Status::QueueFull
            }),
            Status::Ok
        );
        assert!(!delivery_called);
        assert_eq!(bridge.first_delivery_failure(), None);
    }

    #[test]
    fn retains_first_delivery_failure_after_terminal_acknowledgement() {
        let mut bridge = EventBridge::new(TerminalLatch::new());

        bridge.deliver_core(progress(0, PortfolioPhase::SharedArchive), |_frame| {
            Status::QueueFull
        });
        bridge.deliver_core(progress(1, PortfolioPhase::Completed), |_frame| {
            Status::Closing
        });
        let terminal_status = bridge.emit_terminal_and_wait(|_frame, acknowledgement| {
            acknowledgement.acknowledge(Status::Ok);
            Status::Ok
        });

        assert_eq!(terminal_status, Status::Ok);
        assert_eq!(bridge.first_delivery_failure(), Some(Status::QueueFull));
    }
}
