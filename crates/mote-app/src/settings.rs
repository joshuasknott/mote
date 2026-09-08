//! Settings: minimal, polished, persisted to `%APPDATA%\Mote\settings.json`.
//!
//! Everything here is implemented and honoured by the app loop — no dead
//! toggles (see README for the exact behaviour of each).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureSize {
    Small,
    Medium,
    Large,
}

impl CreatureSize {
    /// Sprite-space body radius in px.
    pub fn radius(self) -> f32 {
        match self {
            CreatureSize::Small => 46.0,
            CreatureSize::Medium => 64.0,
            CreatureSize::Large => 86.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            CreatureSize::Small => "Small",
            CreatureSize::Medium => "Medium",
            CreatureSize::Large => "Large",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub size: CreatureSize,
    pub species: mote_core::SpeciesId,
    /// Number of concurrent Motes on desktop (1..=4).
    pub mote_count: u32,
    pub music_reactions: bool,
    pub cursor_interactions: bool,
    pub cpu_reactions: bool,
    /// When false, window top-edges are removed from the world; Mote lives
    /// on the taskbar / screen floor only.
    pub allow_climbing: bool,
    pub reduce_motion: bool,
    pub pause_on_fullscreen: bool,
    pub launch_at_startup: bool,
    pub preferred_monitor: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            size: CreatureSize::Medium,
            species: mote_core::SpeciesId::Peeker,
            mote_count: 1,
            music_reactions: true,
            cursor_interactions: true,
            cpu_reactions: true,
            allow_climbing: true,
            reduce_motion: false,
            pause_on_fullscreen: true,
            launch_at_startup: false,
            preferred_monitor: 0,
        }
    }
}

pub fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Mote").join("settings.json")
}

pub fn log_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Mote").join("mote.log")
}

pub fn load() -> Settings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Settings>(&text) {
            Ok(s) => {
                log::info!("loaded settings from {}", path.display());
                return s;
            }
            Err(e) => log::warn!("bad settings file, using defaults: {e}"),
        },
        Err(_) => log::info!("no settings file, using defaults"),
    }
    Settings::default()
}

pub fn save(s: &Settings) {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(s) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                log::warn!("failed to save settings: {e}");
            }
        }
        Err(e) => log::warn!("failed to serialise settings: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Launch-at-startup via HKCU\...\Run (no admin rights needed).
// The value name is "Mote".

pub fn startup_enabled() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    };
    unsafe {
        let mut hkey = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            KEY_QUERY_VALUE,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        // We only check presence; reading the value is unnecessary.
        let present = windows::Win32::System::Registry::RegQueryValueExW(
            hkey,
            w!("Mote"),
            None,
            None,
            None,
            None,
        )
        .is_ok();
        let _ = RegCloseKey(hkey);
        present
    }
}

pub fn set_startup(enable: bool) {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };
    unsafe {
        let mut hkey = Default::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut hkey,
            None,
        )
        .is_err()
        {
            log::warn!("could not open Run key for startup setting");
            return;
        }
        if enable {
            if let Ok(exe) = std::env::current_exe() {
                let cmd = format!("\"{}\"", exe.display());
                // REG_SZ with nul terminator.
                let mut wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes =
                    { std::slice::from_raw_parts(wide.as_mut_ptr() as *const u8, wide.len() * 2) };
                if RegSetValueExW(hkey, w!("Mote"), None, REG_SZ, Some(bytes)).is_err() {
                    log::warn!("could not write Run value");
                }
            }
        } else if RegDeleteValueW(hkey, w!("Mote")).is_err() {
            log::debug!("Run value already absent");
        }
        let _ = RegCloseKey(hkey);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_sane() {
        let s = Settings::default();
        assert_eq!(s.size, CreatureSize::Medium);
        assert_eq!(s.species, mote_core::SpeciesId::Peeker);
        assert_eq!(s.mote_count, 1);
        assert!(s.music_reactions && s.cursor_interactions && s.cpu_reactions);
    }

    #[test]
    fn roundtrip_json() {
        let s = Settings {
            species: mote_core::SpeciesId::Sprout,
            mote_count: 3,
            ..Default::default()
        };
        let text = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.size, CreatureSize::Medium);
        assert_eq!(back.species, mote_core::SpeciesId::Sprout);
        assert_eq!(back.mote_count, 3);
    }

    #[test]
    fn backward_compatible_deserialization() {
        // Old JSON without species or mote_count
        let old_json = r#"{
            "size": "Small",
            "music_reactions": false,
            "cursor_interactions": true,
            "cpu_reactions": true,
            "allow_climbing": true,
            "reduce_motion": false,
            "pause_on_fullscreen": true,
            "launch_at_startup": false,
            "preferred_monitor": 0
        }"#;
        let s: Settings = serde_json::from_str(old_json).unwrap();
        assert_eq!(s.size, CreatureSize::Small);
        assert_eq!(s.species, mote_core::SpeciesId::Peeker);
        assert_eq!(s.mote_count, 1);
    }
}
