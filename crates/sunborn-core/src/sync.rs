//! Poison-recovering lock guards.
//!
//! Rust's `std` locks return `Err` when the previous holder panicked, and the
//! reflexive `.unwrap()` turns that single failure into a permanent outage: every
//! later caller panics too, on every tenant, until the process restarts.
//!
//! The state these guards protect is plain in-memory maps and counters mutated in
//! short critical sections. A panic leaves that data structurally intact — it is
//! not a partially written file or a half-applied transaction — so recovering the
//! guard is strictly better than aborting the request path.
//!
//! Prefer these helpers over `.read().unwrap()`, `.write().unwrap()`, and
//! `.lock().unwrap()` anywhere on a request, authentication, or accounting path.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Acquire a shared read guard, recovering the state if a writer panicked.
pub fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Acquire an exclusive write guard, recovering the state if a writer panicked.
pub fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Acquire a mutex guard, recovering the state if the holder panicked.
pub fn mutex_lock<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn recovers_read_and_write_after_a_panicking_writer() {
        let lock = Arc::new(RwLock::new(1u32));
        let poisoner = Arc::clone(&lock);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.write().unwrap();
            panic!("simulated writer failure");
        })
        .join();

        // The lock is poisoned now; both helpers must still return usable guards.
        *write_lock(&lock) += 1;
        assert_eq!(*read_lock(&lock), 2);
    }

    #[test]
    fn recovers_mutex_after_a_panicking_holder() {
        let lock = Arc::new(Mutex::new(vec![1u32]));
        let poisoner = Arc::clone(&lock);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("simulated holder failure");
        })
        .join();

        mutex_lock(&lock).push(2);
        assert_eq!(*mutex_lock(&lock), vec![1, 2]);
    }
}
