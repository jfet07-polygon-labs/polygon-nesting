use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use polygon_nesting_core::{CancelReason, CancellationControl};

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

#[derive(Debug, Default)]
pub(crate) struct CancellationRegistry {
    leases: Mutex<HashMap<String, Arc<CancellationLease>>>,
}

impl CancellationRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn register(&self, token: String, lease: Arc<CancellationLease>) -> Result<(), ()> {
        match self.lock().entry(token) {
            Entry::Vacant(entry) => {
                entry.insert(lease);
                Ok(())
            }
            Entry::Occupied(_) => Err(()),
        }
    }
    pub(crate) fn cancel(&self, token: &str, reason: CancelReason) -> bool {
        let Some(lease) = self.lock().get(token).cloned() else {
            return false;
        };
        lease.control().cancel(reason);
        true
    }
    pub(crate) fn remove_if_current(&self, token: &str, lease: &Arc<CancellationLease>) {
        let mut leases = self.lock();
        if leases
            .get(token)
            .is_some_and(|current| Arc::ptr_eq(current, lease))
        {
            leases.remove(token);
        }
    }
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<CancellationLease>>> {
        self.leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(crate) fn native_cancellation_registry() -> Arc<CancellationRegistry> {
    static REGISTRY: OnceLock<Arc<CancellationRegistry>> = OnceLock::new();
    Arc::clone(REGISTRY.get_or_init(|| Arc::new(CancellationRegistry::new())))
}
