use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use polygon_nesting_core::{CancelReason, CancellationControl};

/// One task-owned cancellation state.
///
/// `CancellationControl` owns terminal reason selection so the adapter never
/// duplicates the atomic first-writer-wins state machine.
#[derive(Debug, Default)]
pub(crate) struct CancellationLease {
    control: CancellationControl,
}

impl CancellationLease {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn control(&self) -> &CancellationControl {
        &self.control
    }
}

/// Token-keyed cancellation registrations for one addon process.
///
/// Tokens are opaque invocation identities supplied outside the semantic desktop
/// request. A public desktop job ID is never used to find or replace a lease.
/// AsyncTask completion and environment cleanup must remove only their own
/// pointer-identical lease.
#[derive(Debug, Default)]
pub(crate) struct CancellationRegistry {
    leases_by_invocation_token: Mutex<HashMap<String, Arc<CancellationLease>>>,
}

impl CancellationRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Registers a token exactly once while it remains active.
    ///
    /// A duplicate token is a synchronous adapter error. The original active
    /// lease remains registered and receives any subsequent cancellation.
    pub(crate) fn register(
        &self,
        invocation_token: String,
        lease: Arc<CancellationLease>,
    ) -> Result<(), ()> {
        match self.lock().entry(invocation_token) {
            Entry::Vacant(entry) => {
                entry.insert(lease);
                Ok(())
            }
            Entry::Occupied(_) => Err(()),
        }
    }

    /// Requests cancellation for an active invocation token.
    ///
    /// Returning `true` means that the token was registered. The core control
    /// preserves the first terminal reason if multiple requests arrive.
    pub(crate) fn cancel(&self, invocation_token: &str, reason: CancelReason) -> bool {
        let Some(lease) = self.lock().get(invocation_token).cloned() else {
            return false;
        };
        lease.control().cancel(reason);
        true
    }

    /// Removes a registration only when its completing task owns the lease.
    pub(crate) fn remove_if_current(
        &self,
        invocation_token: &str,
        completing_lease: &Arc<CancellationLease>,
    ) {
        let mut leases = self.lock();
        if leases
            .get(invocation_token)
            .is_some_and(|current| Arc::ptr_eq(current, completing_lease))
        {
            leases.remove(invocation_token);
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<CancellationLease>>> {
        self.leases_by_invocation_token
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Returns the one registry owned for the addon's process lifetime.
///
/// Its contents remain finite because each task and environment cleanup hook
/// releases its active lease through `CancellationRegistry::remove_if_current`.
pub(crate) fn native_cancellation_registry() -> Arc<CancellationRegistry> {
    static REGISTRY: OnceLock<Arc<CancellationRegistry>> = OnceLock::new();
    Arc::clone(REGISTRY.get_or_init(|| Arc::new(CancellationRegistry::new())))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use polygon_nesting_core::CancelReason;

    use super::{CancellationLease, CancellationRegistry};

    #[test]
    fn registry_registers_and_cancels_an_opaque_invocation_token() {
        let registry = CancellationRegistry::new();
        let lease = Arc::new(CancellationLease::new());

        registry
            .register("invocation-token".to_owned(), Arc::clone(&lease))
            .expect("token registers");

        assert!(registry.cancel("invocation-token", CancelReason::Cancelled));
        assert_eq!(lease.control().reason(), Some(CancelReason::Cancelled));
    }

    #[test]
    fn distinct_tokens_remain_independently_cancellable() {
        let registry = CancellationRegistry::new();
        let first = Arc::new(CancellationLease::new());
        let second = Arc::new(CancellationLease::new());
        let first_token = "first-invocation-token";
        let second_token = "second-invocation-token";

        registry
            .register(first_token.to_owned(), Arc::clone(&first))
            .expect("first token registers");
        registry
            .register(second_token.to_owned(), Arc::clone(&second))
            .expect("second token registers");

        assert!(registry.cancel(first_token, CancelReason::Cancelled));
        assert!(registry.cancel(second_token, CancelReason::Deadline));
        assert_eq!(first.control().reason(), Some(CancelReason::Cancelled));
        assert_eq!(second.control().reason(), Some(CancelReason::Deadline));
    }

    #[test]
    fn duplicate_invocation_token_is_rejected_without_replacing_current_lease() {
        let registry = CancellationRegistry::new();
        let current = Arc::new(CancellationLease::new());
        let duplicate = Arc::new(CancellationLease::new());

        registry
            .register("duplicate-token".to_owned(), Arc::clone(&current))
            .expect("initial token registers");
        assert!(registry
            .register("duplicate-token".to_owned(), Arc::clone(&duplicate))
            .is_err());

        assert!(registry.cancel("duplicate-token", CancelReason::Deadline));
        assert_eq!(current.control().reason(), Some(CancelReason::Deadline));
        assert_eq!(duplicate.control().reason(), None);
    }

    #[test]
    fn unknown_invocation_token_cannot_cancel_a_lease() {
        let registry = CancellationRegistry::new();
        let lease = Arc::new(CancellationLease::new());

        registry
            .register("known-token".to_owned(), Arc::clone(&lease))
            .expect("token registers");

        assert!(!registry.cancel("unknown-token", CancelReason::Cancelled));
        assert_eq!(lease.control().reason(), None);
    }

    #[test]
    fn cancellation_delegates_first_terminal_reason_to_core_control() {
        let registry = CancellationRegistry::new();
        let lease = Arc::new(CancellationLease::new());

        registry
            .register("first-reason-token".to_owned(), Arc::clone(&lease))
            .expect("token registers");

        assert!(registry.cancel("first-reason-token", CancelReason::Deadline));
        assert!(registry.cancel("first-reason-token", CancelReason::Cancelled));
        assert_eq!(lease.control().reason(), Some(CancelReason::Deadline));
    }

    #[test]
    fn remove_if_current_requires_pointer_identical_lease() {
        let registry = CancellationRegistry::new();
        let current = Arc::new(CancellationLease::new());
        let other = Arc::new(CancellationLease::new());

        registry
            .register("pointer-token".to_owned(), Arc::clone(&current))
            .expect("token registers");

        registry.remove_if_current("pointer-token", &other);

        assert!(registry.cancel("pointer-token", CancelReason::Cancelled));
        assert_eq!(current.control().reason(), Some(CancelReason::Cancelled));
    }

    #[test]
    fn stale_task_cleanup_cannot_remove_a_newer_invocation_reusing_its_token() {
        let registry = CancellationRegistry::new();
        let stale = Arc::new(CancellationLease::new());
        let current = Arc::new(CancellationLease::new());

        registry
            .register("reused-token".to_owned(), Arc::clone(&stale))
            .expect("stale token registers");
        registry.remove_if_current("reused-token", &stale);
        registry
            .register("reused-token".to_owned(), Arc::clone(&current))
            .expect("replacement token registers after cleanup");

        registry.remove_if_current("reused-token", &stale);

        assert!(registry.cancel("reused-token", CancelReason::Cancelled));
        assert_eq!(current.control().reason(), Some(CancelReason::Cancelled));
        assert_eq!(stale.control().reason(), None);
    }

    #[test]
    fn current_task_cleanup_removes_its_lease() {
        let registry = CancellationRegistry::new();
        let lease = Arc::new(CancellationLease::new());

        registry
            .register("cleanup-token".to_owned(), Arc::clone(&lease))
            .expect("token registers");
        registry.remove_if_current("cleanup-token", &lease);

        assert!(!registry.cancel("cleanup-token", CancelReason::Cancelled));
    }
}
