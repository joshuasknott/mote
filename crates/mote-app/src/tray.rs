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
    SetForegroundWindow, TrackPopupMenu, HICON, ICONINFO, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_NULL,
};

use crate::settings::{CreatureSize, Settings};

pub const TRAY_CALLBACK_MSG: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
const TRAY_ID: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    SleepWake,
    HideShow,
    CallMote,
    ChoosePets,
    ToggleClickThrough,
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
    Some(match id as u32 {
        10 => MenuAction::ChoosePets,
        11 => MenuAction::SleepWake,
        38 => MenuAction::ToggleClickThrough,
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
            item(10, "Choose your pets...");
            item(11, if sleeping { "Wake up" } else { "Sleep now" });
            item(
                12,
                if hidden {
                    "Show pets    Ctrl+Alt+M"
                } else {
                    "Hide pets    Ctrl+Alt+M"
                },
            );
            item(13, "Call Mote here");
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
            toggle(38, "Let clicks pass through pets", s.click_through);
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
                // Windows alpha icons require premultiplied BGRA.
                color[o] = (b / 64) as u8;
                color[o + 1] = (g / 64) as u8;
                color[o + 2] = (r / 64) as u8;
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
        let icon = CreateIconIndirect(&info).ok();
        let _ = windows::Win32::Graphics::Gdi::DeleteObject(hbm_color.into());
        let _ = windows::Win32::Graphics::Gdi::DeleteObject(hbm_mask.into());
        icon
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_routes_picker_and_unobtrusive_controls() {
        assert_eq!(action_from_id(10), Some(MenuAction::ChoosePets));
        assert_eq!(action_from_id(12), Some(MenuAction::HideShow));
        assert_eq!(action_from_id(38), Some(MenuAction::ToggleClickThrough));
        assert_eq!(action_from_id(41), None);
        assert_eq!(action_from_id(99), Some(MenuAction::Quit));
    }
}
