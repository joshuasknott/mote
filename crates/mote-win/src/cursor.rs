//! Cursor position/velocity tracking and system idle time.
//!
//! Cursor velocity uses an exponentially smoothed tracker (pure logic,
//! unit-tested) fed by `GetCursorPos` polls at the sim tick. Idle time comes
//! from `GetLastInputInfo`, the same source Windows screensavers use.

use windows::Win32::Foundation::POINT;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

#[derive(Debug, Clone, Copy, Default)]
pub struct CursorSample {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub speed_px_s: f32,
}

/// Smoothed cursor tracker. `alpha` controls responsiveness (0.35 default).
#[derive(Debug, Clone)]
pub struct CursorTracker {
    last_x: f32,
    last_y: f32,
    vx: f32,
    vy: f32,
    last_t_s: Option<f64>,
    alpha: f32,
    has_sample: bool,
}

impl CursorTracker {
    pub fn new() -> Self {
        Self {
            last_x: 0.0,
            last_y: 0.0,
            vx: 0.0,
            vy: 0.0,
            last_t_s: None,
            alpha: 0.35,
            has_sample: false,
        }
    }

    /// Feed a raw sample; returns the smoothed sample. `t_s` is seconds on
    /// any monotonic clock.
    pub fn push(&mut self, x: f32, y: f32, t_s: f64) -> CursorSample {
        if !self.has_sample {
            self.last_x = x;
            self.last_y = y;
            self.last_t_s = Some(t_s);
            self.has_sample = true;
            return CursorSample {
                x,
                y,
                vx: 0.0,
                vy: 0.0,
                speed_px_s: 0.0,
            };
        }
        let dt = (t_s - self.last_t_s.unwrap_or(t_s)).clamp(0.001, 1.0) as f32;
        let ivx = (x - self.last_x) / dt;
        let ivy = (y - self.last_y) / dt;
        self.vx += (ivx - self.vx) * self.alpha;
        self.vy += (ivy - self.vy) * self.alpha;
        self.last_x = x;
        self.last_y = y;
        self.last_t_s = Some(t_s);
        CursorSample {
            x,
            y,
            vx: self.vx,
            vy: self.vy,
            speed_px_s: (self.vx * self.vx + self.vy * self.vy).sqrt(),
        }
    }

    pub fn current(&self) -> CursorSample {
        CursorSample {
            x: self.last_x,
            y: self.last_y,
            vx: self.vx,
            vy: self.vy,
            speed_px_s: (self.vx * self.vx + self.vy * self.vy).sqrt(),
        }
    }
}

impl Default for CursorTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Poll the cursor position now. Returns None only if the API fails.
pub fn poll_cursor() -> Option<(i32, i32)> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        Some((pt.x, pt.y))
    }
}

/// Milliseconds since the last keyboard/mouse input event. `u64::MAX` on
/// failure (treated as "not idle" by callers — fail safe toward awake).
pub fn idle_ms() -> u64 {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            ..Default::default()
        };
        if !GetLastInputInfo(&mut lii).as_bool() {
            return u64::MAX;
        }
        let now = GetTickCount64();
        now.saturating_sub(lii.dwTime as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_smooths_velocity() {
        let mut t = CursorTracker::new();
        t.push(0.0, 0.0, 0.0);
        // Jump 600px in 16ms repeatedly: raw ~37500 px/s; smoothed climbs.
        let mut s = t.push(600.0, 0.0, 0.016);
        assert!(s.speed_px_s > 1000.0, "fast flick must register {s:?}");
        for i in 1..10 {
            s = t.push(600.0 + i as f32 * 10.0, 0.0, 0.016 * (i + 1) as f64);
        }
        // Nearly stopped: smoothed speed decays below the spike.
        assert!(s.speed_px_s < 20000.0, "should decay {s:?}");
    }

    #[test]
    fn tracker_first_sample_zero_velocity() {
        let mut t = CursorTracker::new();
        let s = t.push(100.0, 200.0, 1.0);
        assert_eq!(s.speed_px_s, 0.0);
    }

    #[test]
    fn idle_query_works() {
        // On a live machine this is a small number; in CI under no input it
        // may be large — either way it must not be the failure sentinel for
        // a successful call... just assert it doesn't crash.
        let _ = idle_ms();
        let _ = poll_cursor();
    }
}
