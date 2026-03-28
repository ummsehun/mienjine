use std::io::{self, IsTerminal};

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalProfile {
    pub is_ghostty: bool,
    pub supports_truecolor: bool,
    pub use_alt_screen: bool,
    pub use_sync_updates: bool,
}

impl TerminalProfile {
    pub fn detect() -> Self {
        let is_ghostty = std::env::var("TERM_PROGRAM")
            .map(|v| v.eq_ignore_ascii_case("ghostty"))
            .unwrap_or(false);
        let use_alt_screen = if is_ghostty {
            true
        } else {
            should_use_alt_screen()
        };
        let use_sync_updates = if is_ghostty {
            true
        } else {
            should_use_sync_updates()
        };
        Self {
            is_ghostty,
            supports_truecolor: supports_truecolor(),
            use_alt_screen,
            use_sync_updates,
        }
    }
}

pub(crate) fn ensure_tty() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("interactive TUI requires a real terminal (TTY). run directly in Ghostty/Terminal.");
    }
    Ok(())
}

pub fn supports_truecolor() -> bool {
    let color_term = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if color_term.contains("truecolor") || color_term.contains("24bit") {
        return true;
    }
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    term.contains("direct")
        || term.contains("kitty")
        || term.contains("wezterm")
        || term.contains("ghostty")
}

fn should_use_alt_screen() -> bool {
    let force_no_alt = std::env::var("GASCII_NO_ALT_SCREEN")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let enable_alt = std::env::var("GASCII_ALT_SCREEN")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    enable_alt && !force_no_alt
}

fn should_use_sync_updates() -> bool {
    let disable = std::env::var("GASCII_NO_SYNC_UPDATES")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    !disable
}
