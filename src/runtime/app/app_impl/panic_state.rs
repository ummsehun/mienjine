use std::{
    fs,
    panic,
    path::PathBuf,
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

/// Capture current runtime state for panic diagnostics.
pub(crate) fn capture_runtime_state() -> Option<String> {
    LAST_RUNTIME_STATE
        .get()
        .and_then(|lock| lock.lock().ok())
        .map(|guard| guard.clone())
        .filter(|s| !s.is_empty())
}

/// Save panic state to a diagnostics file for post-mortem analysis.
pub(crate) fn save_panic_state(state: &str) -> std::io::Result<()> {
    let dir = directories::BaseDirs::new()
        .map(|d| d.data_local_dir().join("terminal-miku3d"))
        .or_else(|| Some(PathBuf::from("/tmp/terminal-miku3d")));

    let Some(dir) = dir else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "could not determine data directory",
        ));
    };

    fs::create_dir_all(&dir)?;
    let path = dir.join("panic_state.log");
    fs::write(path, state)?;
    Ok(())
}

/// Public entry point for setting up the global panic hook.
/// Safe to call multiple times — internally guarded by `Once`.
pub fn setup_panic_hook() {
    install_runtime_panic_hook_once();
}

pub(crate) fn install_runtime_panic_hook_once() {
    PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            cleanup_shm_registry();

            // 1. Log panic info
            eprintln!("[PANIC] {}", panic_info);

            // 2. Capture and log backtrace
            let backtrace = std::backtrace::Backtrace::capture();
            eprintln!("[BACKTRACE]\n{}", backtrace);

            // 3. Save runtime state for post-mortem debugging
            if let Some(state) = capture_runtime_state() {
                let diagnostic = format!(
                    "[PANIC] {}\n\n[BACKTRACE]\n{}\n\n[RUNTIME STATE]\n{}",
                    panic_info, backtrace, state
                );
                if let Err(e) = save_panic_state(&diagnostic) {
                    eprintln!("[PANIC] failed to save state file: {}", e);
                }
            }

            // 4. Original hook
            default_hook(panic_info);
        }));
    });
}
