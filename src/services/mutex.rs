use std::sync::{Mutex, MutexGuard};

/// Recover the guarded value after a panic poisoned the mutex.
pub fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::lock_unpoisoned;
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{Arc, Mutex},
    };

    #[test]
    fn recovers_a_poisoned_mutex() {
        let mutex = Arc::new(Mutex::new(42));
        let poisoned = mutex.clone();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = poisoned.lock().unwrap();
            panic!("poison this mutex");
        }));

        assert!(mutex.lock().is_err());
        assert_eq!(*lock_unpoisoned(&mutex), 42);
    }
}
