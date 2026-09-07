//! Tray icon: Mote lives here when you need it.
//!
//! The icon itself is rendered by the same procedural pipeline as the pet
//! (downscaled to 32px), so the tray and the desktop always match. The
//! context menu is the whole settings UI — deliberately small: every item
//! maps to an implemented behaviour.

use mote_render::{creature::draw_mote, SPRITE_PX};
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, HBITMAP};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyIcon, GetCursorPos, PostMessageW,
    SetForegroundWindow, TrackPopupMenu, HICON, ICONINFO, MF_CHECKED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_NULL,
};

use crate::settings::{CreatureSize, Settings};

pub const TRAY_CALLBACK_MSG: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
const TRAY_ID: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    SleepWake,
    HideShow,
    CallMote,
    SelectSpecies(mote_core::SpeciesId),
    SetMoteCount(u32),
    SizeSmall,
    SizeMedium,
    SizeLarge,
    ToggleMusic,
    ToggleCursor,
    ToggleCpu,
    ToggleClimb,
    ToggleMotion,
    ToggleFullscreen,
    ToggleStartup,
    Quit,
}

fn action_from_id(id: usize) -> Option<MenuAction> {
    if (41..=52).contains(&id) {
        if let Some(sp) = mote_core::SpeciesId::from_index((id - 40) as u8) {
            return Some(MenuAction::SelectSpecies(sp));
        }
    }
    if (61..=64).contains(&id) {
        return Some(MenuAction::SetMoteCount((id - 60) as u32));
    }
    Some(match id as u32 {
        11 => MenuAction::SleepWake,
        12 => MenuAction::HideShow,
        13 => MenuAction::CallMote,
        21 => MenuAction::SizeSmall,
        22 => MenuAction::SizeMedium,
        23 => MenuAction::SizeLarge,
        31 => MenuAction::ToggleMusic,
        32 => MenuAction::ToggleCursor,
        33 => MenuAction::ToggleCpu,
        34 => MenuAction::ToggleClimb,
        35 => MenuAction::ToggleMotion,
        36 => MenuAction::ToggleFullscreen,
        37 => MenuAction::ToggleStartup,
        99 => MenuAction::Quit,
        _ => return None,
    })
}

pub struct TrayIcon {
    hwnd: HWND,
    icon: Option<HICON>,
}

impl TrayIcon {
    pub fn new(hwnd: HWND, species: mote_core::SpeciesId) -> Self {
        let mut t = Self { hwnd, icon: None };
        t.icon = build_mote_icon(species);
        t.add();
        t
    }

    pub fn update_species(&mut self, species: mote_core::SpeciesId) {
        unsafe {
            if let Some(old) = self.icon {
                let _ = DestroyIcon(old);
            }
            self.icon = build_mote_icon(species);
            let mut nid = self.nid();
            nid.uFlags = NIF_ICON;
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    fn nid(&self) -> NOTIFYICONDATAW {
        let mut nid = NOTIFYICONDATAW {
            hWnd: self.hwnd,
            uID: TRAY_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: TRAY_CALLBACK_MSG,
            ..Default::default()
        };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        if let Some(icon) = self.icon {
            nid.hIcon = icon;
        }
        set_tip(&mut nid.szTip, "Mote — a tiny creature lives here");
        nid
    }

    pub fn add(&self) {
        unsafe {
            let nid = self.nid();
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
    }

    pub fn remove(&self) {
        unsafe {
            let nid = self.nid();
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }

    pub fn update_tip(&self, sleeping: bool, hidden: bool) {
        unsafe {
            let mut nid = self.nid();
            nid.uFlags = NIF_TIP;
            let tip = if hidden {
                "Mote — hidden (click to show)"
            } else if sleeping {
                "Mote — sleeping"
            } else {
                "Mote — a tiny creature lives here"
            };
            set_tip(&mut nid.szTip, tip);
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Show the context menu; returns the chosen action, if any.
    pub fn show_menu(&self, s: &Settings, sleeping: bool, hidden: bool) -> Option<MenuAction> {
        unsafe {
            let menu = CreatePopupMenu().ok()?;
            let check = |on: bool| {
                if on {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                }
            };
            let item = |id: u32, text: &str| {
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(menu, MF_STRING, id as usize, PCWSTR(wide.as_ptr()));
            };
            let toggle = |id: u32, text: &str, on: bool| {
                let label = format!("{} {}", if on { "✓" } else { "  " }, text);
                let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(menu, check(on), id as usize, PCWSTR(wide.as_ptr()));
            };
            item(11, if sleeping { "Wake up" } else { "Sleep now" });
            item(12, if hidden { "Show Mote" } else { "Hide Mote" });
            item(13, "Call Mote here");
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

            // Choose Mote submenu (12 species)
            let species_menu = CreatePopupMenu().ok()?;
            for &sp in mote_core::SpeciesId::all() {
                let id = 40 + sp.index() as usize;
                let on = s.species == sp;
                let label = format!("{} ({})", sp.full_name(), sp.description());
                let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(species_menu, check(on), id, PCWSTR(wide.as_ptr()));
            }
            let _ = AppendMenuW(menu, MF_POPUP, species_menu.0 as usize, w!("Choose Mote"));

            // Desktop Cohabitation submenu (1 to 4 Motes)
            let count_menu = CreatePopupMenu().ok()?;
            let counts = [
                (1, "1 Mote (Solo)"),
                (2, "2 Motes (Duo)"),
                (3, "3 Motes (Trio)"),
                (4, "4 Motes (Pack)"),
            ];
            for (cnt, label) in counts {
                let on = s.mote_count == cnt;
                let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(
                    count_menu,
                    check(on),
                    (60 + cnt) as usize,
                    PCWSTR(wide.as_ptr()),
                );
            }
            let _ = AppendMenuW(
                menu,
                MF_POPUP,
                count_menu.0 as usize,
                w!("Desktop Cohabitation"),
            );

            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let size_label = format!("Size: {}", s.size.label());
            let wide: Vec<u16> = size_label
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = AppendMenuW(menu, MF_STRING, 0, PCWSTR(wide.as_ptr()));
            let _ = AppendMenuW(
                menu,
                check(s.size == CreatureSize::Small),
                21,
                w!("    Small"),
            );
            let _ = AppendMenuW(
                menu,
                check(s.size == CreatureSize::Medium),
                22,
                w!("    Medium"),
            );
            let _ = AppendMenuW(
                menu,
                check(s.size == CreatureSize::Large),
                23,
                w!("    Large"),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            toggle(31, "React to music", s.music_reactions);
            toggle(32, "Play with cursor", s.cursor_interactions);
            toggle(33, "React to heavy load", s.cpu_reactions);
            toggle(34, "Climb windows", s.allow_climbing);
            toggle(35, "Reduce motion", s.reduce_motion);
            toggle(36, "Pause in fullscreen games", s.pause_on_fullscreen);
            toggle(37, "Launch at startup", s.launch_at_startup);
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            item(99, "Quit Mote");

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);
            let picked = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                None,
                self.hwnd,
                None,
            );
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(menu);
            let id = picked.0 as usize;
            if id == 0 {
                None
            } else {
                action_from_id(id)
            }
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.remove();
        unsafe {
            if let Some(icon) = self.icon {
                let _ = DestroyIcon(icon);
            }
        }
    }
}

fn set_tip(slot: &mut [u16; 128], text: &str) {
    let wide: Vec<u16> = text.encode_utf16().take(127).collect();
    slot[..wide.len()].copy_from_slice(&wide);
    slot[wide.len()] = 0;
}

/// Render the pet and downscale it to a 32px tray icon.
fn build_mote_icon(species: mote_core::SpeciesId) -> Option<HICON> {
    let big = draw_mote(&mote_render::creature::Pose {
        species,
        ..Default::default()
    });
    const N: usize = 32;
    // Box-downsample 256 -> 32 (factor 8), un-premultiply for CreateBitmap.
    let mut color = vec![0u8; N * N * 4];
    let mut mask_bits = [0u8; 32 * 4];
    for oy in 0..N {
        for ox in 0..N {
            let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
            for dy in 0..8 {
                for dx in 0..8 {
                    let sx = ox * 8 + dx;
                    let sy = oy * 8 + dy;
                    let i = (sy * SPRITE_PX + sx) * 4;
                    // Premultiplied source: accumulate then un-premultiply.
                    r += big[i] as u32;
                    g += big[i + 1] as u32;
                    b += big[i + 2] as u32;
                    a += big[i + 3] as u32;
                }
            }
            let o = (oy * N + ox) * 4;
            if a > 0 {
                // Un-premultiply (average).
                color[o] = (b * 64 / a.max(1)) as u8;
                color[o + 1] = (g * 64 / a.max(1)) as u8;
                color[o + 2] = (r * 64 / a.max(1)) as u8;
                color[o + 3] = (a / 64) as u8;
            }
            // 1bpp mask: 1 = transparent.
            let avg_a = a / 64;
            if avg_a < 128 {
                let byte = oy * 4 + ox / 8;
                mask_bits[byte] |= 0x80 >> (ox % 8);
            }
        }
    }
    unsafe {
        let hbm_color: HBITMAP = CreateBitmap(
            N as i32,
            N as i32,
            1,
            32,
            Some(color.as_ptr() as *const std::ffi::c_void),
        );
        if hbm_color.is_invalid() {
            return None;
        }
        let hbm_mask: HBITMAP = CreateBitmap(
            N as i32,
            N as i32,
            1,
            1,
            Some(mask_bits.as_ptr() as *const std::ffi::c_void),
        );
        if hbm_mask.is_invalid() {
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(hbm_color.into());
            return None;
        }
        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        CreateIconIndirect(&info).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::SpeciesId;

    #[test]
    fn action_from_id_maps_all_12_species() {
        for (i, &sp) in SpeciesId::ALL.iter().enumerate() {
            let id = 41 + i;
            assert_eq!(
                action_from_id(id),
                Some(MenuAction::SelectSpecies(sp)),
                "ID {} must map to species {:?}",
                id,
                sp
            );
        }
        // Verify boundaries: 40 and 53 should not map to species
        assert_ne!(
            action_from_id(40),
            Some(MenuAction::SelectSpecies(SpeciesId::Peeker))
        );
        assert_eq!(action_from_id(40), None);
        assert_eq!(action_from_id(53), None);

        // Explicitly check 01 The Peeker and 12 The Kaiju
        assert_eq!(
            action_from_id(41),
            Some(MenuAction::SelectSpecies(SpeciesId::Peeker))
        );
        assert_eq!(
            action_from_id(52),
            Some(MenuAction::SelectSpecies(SpeciesId::Kaiju))
        );
    }

    #[test]
    fn action_from_id_maps_cohabitation_counts() {
        for count in 1..=4 {
            let id = 60 + count;
            assert_eq!(
                action_from_id(id as usize),
                Some(MenuAction::SetMoteCount(count))
            );
        }
    }
}
