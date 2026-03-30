use std::{
    panic,
    sync::{Mutex, Once, OnceLock},
};

use crate::runtime::graphics_proto::cleanup_shm_registry;

static PANIC_HOOK_ONCE: Once = Once::new();
static LAST_RUNTIME_STATE: OnceLock<Mutex<String>> = OnceLock::new();

pub(crate) fn set_runtime_panic_state(line: String) {
    let lock = LAST_RUNTIME_STATE.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut guard) = lock.lock() {
        *guard = line;
    }
}

pub(crate) fn install_runtime_panic_hook_once() {
    PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            cleanup_shm_registry();
            if let Some(lock) = LAST_RUNTIME_STATE.get() {
                if let Ok(state) = lock.lock() {
                    eprintln!("panic_state: {}", state.as_str());
                }
            }
            default_hook(panic_info);
        }));
    });
}
