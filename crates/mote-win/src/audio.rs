//! System-audio activity sensing without capturing any audio.
//!
//! Privacy-first design: we read only the **peak meter** of the default
//! render endpoint (`IAudioMeterInformation::GetPeakValue`). No audio frames
//! are captured, recorded, retained or transmitted — the API physically
//! cannot yield samples through this interface, only a 0..=1 loudness
//! scalar. That scalar drives Mote's music bounce/dance.
//!
//! `media_playing` prefers the Windows Runtime SMTC session state when the
//! foreground media application publishes one. `Playing` is true and
//! `Paused`, `Stopped` or `Closed` are false. Apps that do not publish an
//! SMTC session use the local peak-meter heuristic instead; this still covers
//! music, videos and games uniformly without collecting media metadata.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_SINGLETHREADED};
use windows_future::{AsyncStatus, IAsyncOperation};

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
    smtc: Option<GlobalSystemMediaTransportControlsSessionManager>,
    /// SMTC manager discovery is retained and polled without blocking the UI
    /// thread. RequestAsync may take an arbitrary amount of time on a cold
    /// Windows media stack, so never call its synchronous `get` here.
    pending_smtc: Option<IAsyncOperation<GlobalSystemMediaTransportControlsSessionManager>>,
    /// Whether this instance successfully initialized the WinRT apartment.
    /// `RoUninitialize` must run on the same thread and only when we own it.
    winrt_initialized: bool,
    /// Smoothed peak 0..=1.
    pub level: f32,
    pub playing: bool,
    active_streak_s: f32,
    silent_streak_s: f32,
    last_poll_s: Option<f64>,
    last_retry_s: f64,
    last_smtc_retry_s: f64,
}

impl AudioMonitor {
    pub fn new() -> Self {
        // WinRT/COM init is per-thread; the app constructs this on its main
        // thread. Failure is non-fatal (SMTC stays None -> meter fallback).
        let winrt_initialized = unsafe { RoInitialize(RO_INIT_SINGLETHREADED).is_ok() };
        let meter = init_meter().ok();
        Self {
            meter,
            smtc: None,
            pending_smtc: None,
            winrt_initialized,
            level: 0.0,
            playing: false,
            active_streak_s: 0.0,
            silent_streak_s: 99.0,
            last_poll_s: None,
            last_retry_s: f64::NEG_INFINITY,
            last_smtc_retry_s: f64::NEG_INFINITY,
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
        let meter_reading = self
            .meter
            .as_ref()
            .map(|meter| unsafe { meter.GetPeakValue() });
        let peak = match meter_reading {
            Some(Ok(value)) => value.clamp(0.0, 1.0),
            Some(Err(error)) => {
                // The endpoint can disappear while a headset or display is
                // unplugged. Drop the stale interface so the refresh below
                // can acquire the new default endpoint.
                log::debug!("audio peak meter unavailable; refreshing endpoint: {error:?}");
                self.meter = None;
                0.0
            }
            None => 0.0,
        };
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
            if self.silent_streak_s >= 3.0 {
                self.active_streak_s = 0.0;
            }
        }
        // Progress SMTC discovery before querying it. Status and GetResults
        // are nonblocking; this keeps the app's timer callback bounded.
        self.poll_smtc_discovery(t_s);

        // SMTC is authoritative when a current session reports a stable
        // status. If no session exists (or a producer returns an error), the
        // peak meter remains the privacy-preserving fallback.
        self.playing = query_smtc_playing(self.smtc.as_ref()).unwrap_or_else(|| {
            classify_level(self.level, self.active_streak_s, self.silent_streak_s)
        });
        // Refresh the default endpoint periodically as the default device can
        // change while the old interface remains callable (for example when
        // headphones are connected or removed).
        if t_s - self.last_retry_s > 10.0 {
            self.last_retry_s = t_s;
            if let Ok(meter) = init_meter() {
                self.meter = Some(meter);
            }
        }
    }

    fn poll_smtc_discovery(&mut self, t_s: f64) {
        if let Some(status) = self
            .pending_smtc
            .as_ref()
            .map(|operation| operation.Status())
        {
            match status {
                Ok(AsyncStatus::Completed) => {
                    if let Some(operation) = self.pending_smtc.take() {
                        match operation.GetResults() {
                            Ok(manager) => self.smtc = Some(manager),
                            Err(error) => {
                                log::debug!("SMTC manager discovery failed: {error:?}");
                                self.last_smtc_retry_s = t_s;
                            }
                        }
                        let _ = operation.Close();
                    }
                }
                Ok(AsyncStatus::Started) => {}
                _ => {
                    if let Some(operation) = self.pending_smtc.take() {
                        let _ = operation.Close();
                    }
                    self.last_smtc_retry_s = t_s;
                }
            }
        }

        // Media sessions can appear after startup. RequestAsync itself only
        // creates the operation; completion is consumed by a later poll.
        if self.winrt_initialized
            && self.smtc.is_none()
            && self.pending_smtc.is_none()
            && t_s - self.last_smtc_retry_s > 10.0
        {
            self.last_smtc_retry_s = t_s;
            match GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
                Ok(operation) => self.pending_smtc = Some(operation),
                Err(error) => log::debug!("SMTC manager request failed: {error:?}"),
            }
        }
    }
}

impl Drop for AudioMonitor {
    fn drop(&mut self) {
        // Release every WinRT/COM interface while the apartment is still
        // initialized. Rust would otherwise drop these fields after this
        // method returns, which is too late for RoUninitialize.
        if let Some(operation) = self.pending_smtc.take() {
            let _ = operation.Cancel();
            let _ = operation.Close();
        }
        let _ = self.smtc.take();
        let _ = self.meter.take();
        if self.winrt_initialized {
            // The monitor is owned and dropped by the app's main thread,
            // matching the apartment on which RoInitialize ran.
            unsafe { RoUninitialize() };
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

fn query_smtc_playing(
    manager: Option<&GlobalSystemMediaTransportControlsSessionManager>,
) -> Option<bool> {
    let session = manager?.GetCurrentSession().ok()?;
    let status = session.GetPlaybackInfo().ok()?.PlaybackStatus().ok()?;
    classify_smtc_status(status)
}

fn classify_smtc_status(
    status: GlobalSystemMediaTransportControlsSessionPlaybackStatus,
) -> Option<bool> {
    match status {
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => Some(true),
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed
        | GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused
        | GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => Some(false),
        _ => None,
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
    fn smtc_status_is_authoritative_when_stable() {
        assert_eq!(
            classify_smtc_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing),
            Some(true)
        );
        assert_eq!(
            classify_smtc_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused),
            Some(false)
        );
        assert_eq!(
            classify_smtc_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped),
            Some(false)
        );
        assert_eq!(
            classify_smtc_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing),
            None
        );
    }

    #[test]
    fn monitor_constructs_without_device() {
        // Must never panic, even headless.
        let _ = AudioMonitor::new();
    }
}
