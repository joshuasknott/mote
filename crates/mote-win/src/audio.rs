//! System-audio activity sensing without capturing any audio.
//!
//! Privacy-first design: we read only the **peak meter** of the default
//! render endpoint (`IAudioMeterInformation::GetPeakValue`). No audio frames
//! are captured, recorded, retained or transmitted — the API physically
//! cannot yield samples through this interface, only a 0..=1 loudness
//! scalar. That scalar drives Mote's music bounce/dance.
//!
//! `media_playing` is derived: sustained meter activity (> 0.02 for ~2 s)
//! counts as playback; it clears after ~3 s of silence. A full WinRT SMTC
//! (SystemMediaTransportControls) integration for exact play/stop metadata
//! is tracked as future work in BUILD_PLAN — the current heuristic already
//! covers music, videos and games uniformly.

use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};

/// Classify a smoothed peak level into (playing, level).
/// Pure function so the hysteresis is unit-testable.
pub fn classify_level(smoothed: f32, active_streak_s: f32, silent_streak_s: f32) -> bool {
    let _ = smoothed;
    // Hysteresis: need 2 s of sound to start, 3 s of silence to stop.
    if active_streak_s >= 2.0 {
        silent_streak_s < 3.0
    } else {
        false
    }
}

pub struct AudioMonitor {
    meter: Option<IAudioMeterInformation>,
    /// Smoothed peak 0..=1.
    pub level: f32,
    pub playing: bool,
    active_streak_s: f32,
    silent_streak_s: f32,
    last_poll_s: Option<f64>,
    last_retry_s: f64,
}

impl AudioMonitor {
    pub fn new() -> Self {
        // COM init is per-thread; the app constructs this on its main
        // thread. Failure is non-fatal (meter stays None -> silence).
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let meter = init_meter().ok();
        Self {
            meter,
            level: 0.0,
            playing: false,
            active_streak_s: 0.0,
            silent_streak_s: 99.0,
            last_poll_s: None,
            last_retry_s: f64::NEG_INFINITY,
        }
    }

    pub fn available(&self) -> bool {
        self.meter.is_some()
    }

    /// Poll the peak meter. Call at ~10 Hz; `t_s` is seconds, monotonic.
    pub fn poll(&mut self, t_s: f64) {
        let dt = match self.last_poll_s {
            Some(last) => (t_s - last).clamp(0.005, 5.0) as f32,
            None => 0.1,
        };
        self.last_poll_s = Some(t_s);
        let peak = self
            .meter
            .as_ref()
            .and_then(|m| unsafe { m.GetPeakValue().ok() })
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        // Fast attack, slow release — like a VU meter.
        if peak > self.level {
            self.level += (peak - self.level) * 0.6;
        } else {
            self.level += (peak - self.level) * (dt * 3.0).min(1.0);
        }
        if self.level > 0.02 {
            self.active_streak_s += dt;
            self.silent_streak_s = 0.0;
        } else {
            self.silent_streak_s += dt;
            if self.silent_streak_s > 1.0 {
                self.active_streak_s = 0.0;
            }
        }
        self.playing = classify_level(self.level, self.active_streak_s, self.silent_streak_s);
        // If the device vanished (unplugged headset etc.), retry init rarely.
        if self.meter.is_none() && t_s - self.last_retry_s > 10.0 {
            self.last_retry_s = t_s;
            self.meter = init_meter().ok();
        }
    }
}

impl Default for AudioMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn init_meter() -> windows::core::Result<IAudioMeterInformation> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        let meter: IAudioMeterInformation = device.Activate(CLSCTX_ALL, None)?;
        Ok(meter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hysteresis_needs_sustained_sound() {
        assert!(!classify_level(0.5, 0.5, 0.0));
        assert!(classify_level(0.5, 2.5, 0.0));
        assert!(!classify_level(0.0, 0.0, 5.0));
    }

    #[test]
    fn monitor_constructs_without_device() {
        // Must never panic, even headless.
        let _ = AudioMonitor::new();
    }
}
