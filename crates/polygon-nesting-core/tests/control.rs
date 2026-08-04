use std::sync::{Arc, Barrier};
use std::thread;

use polygon_nesting_core::{CancelReason, CancellationControl};

#[test]
fn cancellation_retains_first_cancelled_writer() {
    let control = CancellationControl::new();

    assert!(control.cancel(CancelReason::Cancelled));
    assert!(!control.cancel(CancelReason::Deadline));
    assert_eq!(control.reason(), Some(CancelReason::Cancelled));
    assert_eq!(control.checkpoint(), Err(CancelReason::Cancelled));
}

#[test]
fn cancellation_retains_first_deadline_writer() {
    let control = CancellationControl::new();

    assert!(control.cancel(CancelReason::Deadline));
    assert!(!control.cancel(CancelReason::Cancelled));
    assert_eq!(control.reason(), Some(CancelReason::Deadline));
    assert_eq!(control.checkpoint(), Err(CancelReason::Deadline));
}

#[test]
fn cancellation_concurrent_writers_preserve_one_barrier_winner() {
    let control = Arc::new(CancellationControl::new());
    let barrier = Arc::new(Barrier::new(3));
    let cancelled_control = Arc::clone(&control);
    let cancelled_barrier = Arc::clone(&barrier);
    let cancelled = thread::spawn(move || {
        cancelled_barrier.wait();
        cancelled_control.cancel(CancelReason::Cancelled)
    });
    let deadline_control = Arc::clone(&control);
    let deadline_barrier = Arc::clone(&barrier);
    let deadline = thread::spawn(move || {
        deadline_barrier.wait();
        deadline_control.cancel(CancelReason::Deadline)
    });

    barrier.wait();
    let cancelled_won = cancelled.join().unwrap();
    let deadline_won = deadline.join().unwrap();

    assert_ne!(cancelled_won, deadline_won);
    assert!(matches!(
        control.reason(),
        Some(CancelReason::Cancelled | CancelReason::Deadline)
    ));
    assert_eq!(control.checkpoint(), Err(control.reason().unwrap()));
}
