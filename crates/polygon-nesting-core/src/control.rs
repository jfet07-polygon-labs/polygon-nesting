//! Job cancellation control.
//!
//! Cancellation is an atomic first-writer-wins state machine. The same state
//! is exposed to callers and bridged into NFP/IFP checkpoints with typed abort
//! reasons.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::nfp_ifp::{
    NfpIfpAbortReason, NfpIfpCheckpointPhase, NfpIfpControl, NfpIfpControlAbortError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Cancelled,
    Deadline,
}

const RUNNING: u8 = 0;
const CANCELLED: u8 = 1;
const DEADLINE: u8 = 2;

#[derive(Debug)]
pub struct CancellationControl {
    state: AtomicU8,
}

impl Default for CancellationControl {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationControl {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(RUNNING),
        }
    }

    pub fn cancel(&self, reason: CancelReason) -> bool {
        let terminal = match reason {
            CancelReason::Cancelled => CANCELLED,
            CancelReason::Deadline => DEADLINE,
        };
        self.state
            .compare_exchange(RUNNING, terminal, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn reason(&self) -> Option<CancelReason> {
        match self.state.load(Ordering::Acquire) {
            RUNNING => None,
            CANCELLED => Some(CancelReason::Cancelled),
            DEADLINE => Some(CancelReason::Deadline),
            _ => unreachable!("invalid cancellation state"),
        }
    }

    pub fn checkpoint(&self) -> Result<(), CancelReason> {
        self.reason().map_or(Ok(()), Err)
    }
}

impl NfpIfpControl for CancellationControl {
    fn checkpoint(&mut self, _phase: NfpIfpCheckpointPhase) -> Result<(), NfpIfpControlAbortError> {
        CancellationControl::checkpoint(self).map_err(|reason| NfpIfpControlAbortError {
            reason: match reason {
                CancelReason::Cancelled => NfpIfpAbortReason::Cancelled,
                CancelReason::Deadline => NfpIfpAbortReason::Deadline,
            },
            message: match reason {
                CancelReason::Cancelled => "cancelled".to_owned(),
                CancelReason::Deadline => "deadline exceeded".to_owned(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_bridges_to_nfp_ifp_control() {
        let mut control = CancellationControl::new();
        assert!(control.cancel(CancelReason::Deadline));

        let result =
            NfpIfpControl::checkpoint(&mut control, NfpIfpCheckpointPhase::CandidatePoints);
        let error = result.unwrap_err();
        assert_eq!(error.reason, NfpIfpAbortReason::Deadline);
    }
}
