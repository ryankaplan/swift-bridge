//! Main Thread Async Support
//! 
//! This module provides async support for apps using swift-bridge as a static library.
//! All async code runs on the main thread to ensure thread-safety with UI frameworks.
//!
//! Usage from the containing app:
//! 1. Call `get_async_runtime()` once to get the runtime instance
//! 2. Call `swift_bridge_update_runtime()` every frame (e.g. in CADisplayLink or timer)
//! 
//! The runtime processes tasks in this order:
//! 1. Takes pending tasks from the queue
//! 2. Executes them on the main thread
//! 3. Yields to allow other work

use std::future::Future;
use std::pin::Pin;
use std::cell::RefCell;
use tokio::runtime::Runtime;

thread_local! {
    static ASYNC_RUNTIME: RefCell<Option<Runtime>> = RefCell::new(None);
    static TASKS: RefCell<Vec<AsyncFnToSpawn>> = RefCell::new(Vec::new());
}

// Tasks don't need Send because they never leave the main thread
type AsyncFnToSpawn = Pin<Box<dyn Future<Output = ()> + 'static>>;

#[doc(hidden)]
pub struct SwiftCallbackWrapper(pub *mut std::ffi::c_void);


pub fn spawn_task(task: AsyncFnToSpawn) {
    TASKS.with(|tasks| tasks.borrow_mut().push(task));
}

/// Updates the runtime - must be called regularly on the main thread
/// (e.g. every frame via CADisplayLink)
#[no_mangle]
pub extern "C" fn swift_bridge_update_runtime() {
    ASYNC_RUNTIME.with(|runtime_cell| {
        // Initialize runtime if needed
        if runtime_cell.borrow().is_none() {
            *runtime_cell.borrow_mut() = Some(Runtime::new().unwrap());
        }

        // Process pending tasks
        if let Some(rt) = &*runtime_cell.borrow() {
            rt.block_on(async {
                while let Some(task) = TASKS.with(|t| t.borrow_mut().pop()) {
                    task.await;
                }
                tokio::task::yield_now().await;
            });
        }
    });
}
