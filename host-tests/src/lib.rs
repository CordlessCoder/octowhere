#![allow(dead_code)]
#![allow(clippy::drop_non_drop)]

#[path = "../../firmware/src/util.rs"]
pub mod util;

/// `SwapThread` must not cross a thread boundary when its value is not `Send`.
///
/// ```compile_fail,E0277
/// use octowhere_host_tests::util::Swap;
/// use std::sync::{Mutex, MutexGuard};
///
/// fn assert_send<T: Send>(_: T) {}
/// fn assert_sync<T: Sync>() {}
/// let first_lock = Mutex::new(());
/// let second_lock = Mutex::new(());
/// let mut swap = Swap::new(first_lock.lock().unwrap(), second_lock.lock().unwrap());
/// let (first, _second) = swap.split();
/// assert_sync::<MutexGuard<'_, ()>>();
/// assert_send(first);
/// ```
pub const SWAP_SEND_BOUND: () = ();

#[path = "../../firmware/src/gnss_time.rs"]
pub mod gnss_time;

#[path = "../../firmware/src/settings_queue.rs"]
pub mod settings_queue;

#[cfg(test)]
mod settings_writes;

#[cfg(test)]
mod gnss;

pub use octowhere_ui::ui::compass;
