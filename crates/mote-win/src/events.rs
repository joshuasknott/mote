//! Event-driven window-change notifications via `SetWinEventHook`.
//!
//! Polling `EnumWindows` at 60 Hz would be wasteful; instead we install an
//! out-of-context WinEvent hook for move/size/destroy/foreground/minimise
//! events. The callback only bumps an atomic counter — the app then rebuilds
//! the world lazily (and still refreshes on a slow 2 s fallback so a missed
//! event can never desync Mote permanently).

use std::sync::atomic::{AtomicU64, Ordering};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_DESTROY, EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};

static CHANGE_COUNTER: AtomicU64 = AtomicU64::new(1);

unsafe extern "system" fn hook_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    CHANGE_COUNTER.fetch_add(1, Ordering::Relaxed);
}

/// RAII WinEvent hook. Installs on creation, removes on drop.
pub struct WinEventListener {
    hooks: Vec<HWINEVENTHOOK>,
    last_seen: u64,
}

impl WinEventListener {
    pub fn install() -> Self {
        let mut hooks = Vec::new();
        unsafe {
            // Object range: destroy/move/size/reorder/show/hide. (Event
            // constants must satisfy min <= max, so system events get their
            // own range below.)
            let hook = SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_LOCATIONCHANGE,
                None,
                Some(hook_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if !hook.is_invalid() {
                hooks.push(hook);
            }
            // Minimise start/end.
            let min = SetWinEventHook(
                EVENT_SYSTEM_MINIMIZESTART,
                EVENT_SYSTEM_MINIMIZEEND,
                None,
                Some(hook_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if !min.is_invalid() {
                hooks.push(min);
            }
            // Foreground changes live above that range; hook separately.
            let fg = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(hook_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if !fg.is_invalid() {
                hooks.push(fg);
            }
        }
        Self {
            hooks,
            last_seen: CHANGE_COUNTER.load(Ordering::Relaxed),
        }
    }

    /// True if any hooked event fired since the last call.
    pub fn poll_changed(&mut self) -> bool {
        let cur = CHANGE_COUNTER.load(Ordering::Relaxed);
        if cur != self.last_seen {
            self.last_seen = cur;
            true
        } else {
            false
        }
    }
}

impl Drop for WinEventListener {
    fn drop(&mut self) {
        unsafe {
            for h in self.hooks.drain(..) {
                let _ = UnhookWinEvent(h);
            }
        }
    }
}

// SAFETY: hooks are process-global registrations; the struct only carries
// opaque handles and an integer. Callbacks never touch self.
unsafe impl Send for WinEventListener {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_installs_and_polls() {
        let mut l = WinEventListener::install();
        // No assertion on events (environment-dependent); just exercise it.
        let _ = l.poll_changed();
        let _ = l.poll_changed();
    }
}
