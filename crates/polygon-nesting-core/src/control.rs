//! Core cancellation API.
//!
//! This module defines the Task 23 service seam. Cancellation state and
//! checkpoint behavior are implemented by the execution service in Task 24.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Cancelled,
    Deadline,
}

#[derive(Debug, Default)]
pub struct CancellationControl {
    _private: (),
}

impl CancellationControl {
    pub const fn new() -> Self {
        Self { _private: () }
    }

    pub fn cancel(&self, _reason: CancelReason) -> bool {
        todo!("Task 24 cancellation behavior")
    }

    pub fn reason(&self) -> Option<CancelReason> {
        todo!("Task 24 cancellation behavior")
    }

    pub fn checkpoint(&self) -> Result<(), CancelReason> {
        todo!("Task 24 cancellation behavior")
    }
}
