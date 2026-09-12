//! Native pet selection window.
//!
//! The picker deliberately lives outside [`crate::app::App`].  It is a small,
//! temporary Win32 window with its own state, GDI paint resources, and input
//! handling.  A committed selection is sent back to the owner as a private
//! `WM_APP` message; the owner can then call [`take_result`].  This keeps the
//! desktop pet's overlay loop free of modal state and means opening the picker
//! never changes the pet's behaviour until the user presses "Bring them home".

use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use mote_core::SpeciesId;
use mote_render::draw_portrait;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AlphaBlend, BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    CreateRoundRectRgn, DeleteDC, DeleteObject, EndPaint, FillRect, FrameRgn, GetMonitorInfoW,
    SelectObject, SetBkMode, SetTextColor, SetWindowRgn, AC_SRC_ALPHA, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DEFAULT_PITCH,
    DIB_RGB_COLORS, DRAW_TEXT_FORMAT, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK,
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FW_BOLD, FW_NORMAL, HBITMAP, HBRUSH, HDC, HFONT,
    MONITORINFO, MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, SRCCOPY, TRANSPARENT,
};
use windows::Win32::Graphics::Gdi::{InvalidateRect, MonitorFromWindow, UpdateWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_BACK, VK_DOWN, VK_ESCAPE, VK_LEFT,
    VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW, IsWindow,
    LoadCursorW, PostMessageW, RegisterClassExW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    ShowWindow, GWLP_USERDATA, IDC_ARROW, SW_SHOW, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE,
    WM_NCDESTROY, WM_PAINT, WM_SETCURSOR, WM_SIZE, WM_TIMER, WNDCLASSEXW, WS_BORDER,
    WS_EX_APPWINDOW, WS_EX_DLGMODALFRAME, WS_POPUP,
};

#[allow(non_snake_case)]
fn RGB(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

/// Message posted to the owner after the picker closes.
pub const WM_APP_OPEN_PICKER: u32 = 0x8000 + 0x14D;
pub const WM_APP_PICKER_RESULT: u32 = 0x8000 + 0x14E;
const PICKER_TIMER: usize = 0x4D1;
const CLASS_NAME: PCWSTR = w!("MotePetPicker");
const MAX_PETS: usize = 4;
const ROSTER_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerResult {
    pub pets: Vec<SpeciesId>,
    pub cancelled: bool,
    pub quiet: bool,
    pub reduced_motion: bool,
}

static RESULTS: OnceLock<Mutex<HashMap<isize, PickerResult>>> = OnceLock::new();
static OPEN: OnceLock<Mutex<HashMap<isize, isize>>> = OnceLock::new();

fn results() -> &'static Mutex<HashMap<isize, PickerResult>> {
    RESULTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn open_windows() -> &'static Mutex<HashMap<isize, isize>> {
    OPEN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Read and remove the most recent result belonging to an owner window.
pub fn take_result(owner: HWND) -> Option<PickerResult> {
    results().lock().ok()?.remove(&(owner.0 as isize))
}

/// Close a picker belonging to `owner` without changing the running pack.
/// Useful when the tray/app is shutting down.
pub fn close_picker(owner: HWND) {
    let raw_hwnd = open_windows()
        .lock()
        .ok()
        .and_then(|mut open| open.remove(&(owner.0 as isize)));
    if let Some(raw_hwnd) = raw_hwnd {
        let hwnd = HWND(raw_hwnd as *mut std::ffi::c_void);
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

/// Open the native picker, or focus the existing picker for this owner.
///
/// `selected` is cloned and normalised to four unique species.  `quiet` and
/// `reduced_motion` are presentation hints only; the picker never plays sound
/// and does not touch the running `App`.
pub fn show_picker(
    owner: HWND,
    selected: Vec<SpeciesId>,
    quiet: bool,
    reduced_motion: bool,
) -> windows::core::Result<()> {
    let key = owner.0 as isize;
    if let Ok(open) = open_windows().lock() {
        if let Some(&raw_hwnd) = open.get(&key) {
            let hwnd = HWND(raw_hwnd as *mut std::ffi::c_void);
            unsafe {
                if IsWindow(Some(hwnd)).as_bool() {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = SetForegroundWindow(hwnd);
                    let _ = SetFocus(Some(hwnd));
                    return Ok(());
                }
            }
        }
    }

    unsafe {
        let instance: HINSTANCE =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?.into();
        register_class(instance)?;
        let (width, height, x, y) = picker_bounds(owner);
        let mut state = Box::new(PickerState::new(owner, selected, quiet, reduced_motion));
        state.width = width;
        state.height = height;
        let state_ptr = (&mut *state as *mut PickerState).cast::<std::ffi::c_void>();
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_DLGMODALFRAME,
            CLASS_NAME,
            w!("Choose your Mote"),
            WS_POPUP | WS_BORDER,
            x,
            y,
            width,
            height,
            // Keep this as a separate top-level window. The overlay owner is
            // hidden during quiet/fullscreen cadence changes; making it the
            // Win32 owner would hide the picker along with the pet.
            None,
            None,
            Some(instance),
            Some(state_ptr),
        )?;
        let _ = Box::into_raw(state);
        if let Ok(mut open) = open_windows().lock() {
            open.insert(key, hwnd.0 as isize);
        }
        let _ = SetTimer(
            Some(hwnd),
            PICKER_TIMER,
            if reduced_motion { 120 } else { 33 },
            None,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        let _ = UpdateWindow(hwnd);
        Ok(())
    }
}

unsafe fn register_class(instance: HINSTANCE) -> windows::core::Result<()> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    if REGISTERED.get().is_some() {
        return Ok(());
    }
    let cursor = LoadCursorW(None, IDC_ARROW)?;
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: Default::default(),
        lpfnWndProc: Some(picker_wndproc),
        hInstance: instance,
        hCursor: cursor,
        hbrBackground: HBRUSH::default(),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    let atom = RegisterClassExW(&class);
    if atom == 0 {
        let error = windows::Win32::Foundation::GetLastError();
        // ERROR_CLASS_ALREADY_EXISTS is harmless when another picker instance
        // registered the class between our OnceLock check and this call.
        if error.0 != 1410 {
            return Err(windows::core::Error::from_win32());
        }
    }
    let _ = REGISTERED.set(());
    Ok(())
}

fn picker_bounds(owner: HWND) -> (i32, i32, i32, i32) {
    unsafe {
        let monitor = MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if monitor.0.is_null() || !GetMonitorInfoW(monitor, &mut mi).as_bool() {
            return (960, 620, 160, 100);
        }
        let work = mi.rcWork;
        // Keep the window comfortable on a small laptop and bounded to the
        // work area so it never hides the taskbar or spills off-screen.
        let width = 1040.min((work.right - work.left - 32).max(720));
        let height = 680.min((work.bottom - work.top - 32).max(500));
        let x = work.left + ((work.right - work.left - width) / 2).max(0);
        let y = work.top + ((work.bottom - work.top - height) / 2).max(0);
        (width, height, x, y)
    }
}

struct PickerState {
    owner: HWND,
    pets: Vec<SpeciesId>,
    species: Vec<SpeciesId>,
    sprites: Vec<Sprite>,
    cursor: usize,
    focus: FocusTarget,
    hot: Option<usize>,
    quiet: bool,
    reduced_motion: bool,
    phase: f32,
    last_tick: Instant,
    width: i32,
    height: i32,
    pointer_down: bool,
    pressed: Option<FocusTarget>,
    buffer: BackBuffer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusTarget {
    Roster(usize),
    Quiet,
    Motion,
    Lineup(usize),
    BringHome,
    Cancel,
}

impl FocusTarget {
    fn focus_index(self) -> usize {
        match self {
            Self::Roster(index) => index.min(5),
            Self::Quiet => 6,
            Self::Motion => 7,
            Self::Lineup(index) => 8 + index.min(3),
            Self::BringHome => 12,
            Self::Cancel => 13,
        }
    }

    fn from_focus_index(index: usize) -> Self {
        match index % 14 {
            0..=5 => Self::Roster(index),
            6 => Self::Quiet,
            7 => Self::Motion,
            8..=11 => Self::Lineup(index - 8),
            12 => Self::BringHome,
            _ => Self::Cancel,
        }
    }
}

impl PickerState {
    fn new(owner: HWND, selected: Vec<SpeciesId>, quiet: bool, reduced_motion: bool) -> Self {
        let species: Vec<_> = SpeciesId::all()
            .iter()
            .copied()
            .take(ROSTER_COUNT)
            .collect();
        let mut pets = Vec::with_capacity(MAX_PETS);
        for pet in selected {
            if species.contains(&pet) && !pets.contains(&pet) && pets.len() < MAX_PETS {
                pets.push(pet);
            }
        }
        let sprites = species.iter().copied().map(Sprite::new).collect();
        Self {
            owner,
            pets,
            species,
            sprites,
            cursor: 0,
            focus: FocusTarget::Roster(0),
            hot: None,
            quiet,
            reduced_motion,
            phase: 0.0,
            last_tick: Instant::now(),
            width: 1040,
            height: 680,
            pointer_down: false,
            pressed: None,
            buffer: BackBuffer::new(1040, 680),
        }
    }

    fn selected(&self, index: usize) -> bool {
        self.pets.contains(&self.species[index])
    }

    fn toggle_cursor(&mut self) {
        let species = self.species[self.cursor];
        if let Some(pos) = self.pets.iter().position(|&p| p == species) {
            self.pets.remove(pos);
        } else if self.pets.len() < MAX_PETS {
            self.pets.push(species);
        }
    }

    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let col = (self.cursor % 3) as i32;
        let row = (self.cursor / 3) as i32;
        let next_col = (col + dx).clamp(0, 2);
        let next_row = (row + dy).clamp(0, 1);
        self.cursor = (next_row * 3 + next_col) as usize;
        self.focus = FocusTarget::Roster(self.cursor);
    }

    fn commit(&self, hwnd: HWND) {
        if self.pets.is_empty() {
            return;
        }
        if let Ok(mut map) = results().lock() {
            map.insert(
                self.owner.0 as isize,
                PickerResult {
                    pets: self.pets.clone(),
                    cancelled: false,
                    quiet: self.quiet,
                    reduced_motion: self.reduced_motion,
                },
            );
        }
        unsafe {
            let _ = PostMessageW(Some(self.owner), WM_APP_PICKER_RESULT, WPARAM(0), LPARAM(0));
            let _ = DestroyWindow(hwnd);
        }
    }

    fn cancel(&self, hwnd: HWND) {
        if let Ok(mut map) = results().lock() {
            map.insert(
                self.owner.0 as isize,
                PickerResult {
                    pets: self.pets.clone(),
                    cancelled: true,
                    quiet: self.quiet,
                    reduced_motion: self.reduced_motion,
                },
            );
        }
        unsafe {
            let _ = PostMessageW(Some(self.owner), WM_APP_PICKER_RESULT, WPARAM(0), LPARAM(0));
            let _ = DestroyWindow(hwnd);
        }
    }
}

struct Sprite {
    dc: HDC,
    dib: HBITMAP,
}

/// Persistent paint target. Painting every card directly into the window
/// causes GDI to reveal partially updated footer/header regions while the
/// picker is animating; a single BitBlt keeps each frame coherent.
struct BackBuffer {
    dc: HDC,
    dib: HBITMAP,
    width: i32,
    height: i32,
}

impl BackBuffer {
    fn new(width: i32, height: i32) -> Self {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let dc = CreateCompatibleDC(None);
            let mut bits = std::ptr::null_mut();
            let dib = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .unwrap_or_default();
            let _ = SelectObject(dc, dib.into());
            Self {
                dc,
                dib,
                width,
                height,
            }
        }
    }

    fn ensure(&mut self, width: i32, height: i32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        let replacement = Self::new(width, height);
        *self = replacement;
    }
}

impl Drop for BackBuffer {
    fn drop(&mut self) {
        unsafe {
            if !self.dc.0.is_null() {
                let _ = DeleteDC(self.dc);
            }
            if !self.dib.0.is_null() {
                let _ = DeleteObject(self.dib.into());
            }
        }
    }
}

impl Sprite {
    fn new(species: SpeciesId) -> Self {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 256,
                biHeight: -256,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let frame = draw_portrait(species, 256, 256);
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        unsafe {
            let dib = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .unwrap_or_default();
            let dc = CreateCompatibleDC(None);
            if !bits.is_null() {
                let dst = std::slice::from_raw_parts_mut(bits.cast::<u8>(), frame.len());
                for (out, input) in dst
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(frame.as_chunks::<4>().0)
                {
                    // draw_mote is premultiplied RGBA; layered AlphaBlend wants
                    // premultiplied BGRA in a top-down DIB.
                    *out = [input[2], input[1], input[0], input[3]];
                }
            }
            let _ = SelectObject(dc, dib.into());
            Self { dc, dib }
        }
    }
}

impl Drop for Sprite {
    fn drop(&mut self) {
        unsafe {
            if !self.dc.0.is_null() {
                let _ = DeleteDC(self.dc);
            }
            if !self.dib.0.is_null() {
                let _ = DeleteObject(self.dib.into());
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Layout {
    preview: RECT,
    cards: [RECT; ROSTER_COUNT],
    lineup: [RECT; MAX_PETS],
    bring_home: RECT,
    cancel: RECT,
    quiet_toggle: RECT,
    motion_toggle: RECT,
}

fn layout(width: i32, height: i32) -> Layout {
    let pad = 32;
    let preview = RECT {
        left: pad,
        top: 150,
        right: (width * 42 / 100).max(330),
        bottom: height - 124,
    };
    let right = (width * 46 / 100).max(preview.right + 24);
    // At the minimum picker width the roster has less room than the desktop
    // card width. Keep the cards inside the client area instead of letting
    // the final column spill past the rounded window edge. The wider layout
    // retains its established dimensions.
    let card_w = if width < 1000 {
        ((width - right - pad - 24) / 3).max(96)
    } else {
        ((width - right - pad - 20) / 3).max(118)
    };
    let card_h = ((height - 150 - 92 - 12) / 2).clamp(90, 190);
    let mut cards = [RECT::default(); ROSTER_COUNT];
    for (i, card) in cards.iter_mut().enumerate() {
        let col = (i % 3) as i32;
        let row = (i / 3) as i32;
        let left = right + col * (card_w + 12);
        let top = 150 + row * (card_h + 12);
        *card = RECT {
            left,
            top,
            right: left + card_w,
            bottom: top + card_h,
        };
    }
    let lineup_left = pad;
    let lineup_top = height - 92;
    let slot_w = 84;
    let lineup = std::array::from_fn(|i| RECT {
        left: lineup_left + i as i32 * (slot_w + 10),
        top: lineup_top,
        right: lineup_left + i as i32 * (slot_w + 10) + slot_w,
        bottom: lineup_top + 56,
    });
    Layout {
        preview,
        cards,
        lineup,
        bring_home: RECT {
            left: width - 244,
            top: height - 88,
            right: width - pad,
            bottom: height - 38,
        },
        cancel: RECT {
            left: width - 340,
            top: 38,
            right: width - 244,
            bottom: 72,
        },
        quiet_toggle: RECT {
            left: 32,
            top: 108,
            right: 144,
            bottom: 136,
        },
        motion_toggle: RECT {
            left: 154,
            top: 108,
            right: 310,
            bottom: 136,
        },
    }
}

unsafe fn fill_round(hdc: HDC, rect: RECT, colour: COLORREF, radius: i32) {
    let brush = windows::Win32::Graphics::Gdi::CreateSolidBrush(colour);
    let region = CreateRoundRectRgn(rect.left, rect.top, rect.right, rect.bottom, radius, radius);
    // FillRgn preserves the rounded silhouette; the window background is
    // already painted before cards are drawn.
    let _ = windows::Win32::Graphics::Gdi::FillRgn(hdc, region, brush);
    let _ = DeleteObject(region.into());
    let _ = DeleteObject(brush.into());
}

unsafe fn frame_round(hdc: HDC, rect: RECT, colour: COLORREF, radius: i32, width: i32) {
    let region = CreateRoundRectRgn(rect.left, rect.top, rect.right, rect.bottom, radius, radius);
    let brush = windows::Win32::Graphics::Gdi::CreateSolidBrush(colour);
    let _ = FrameRgn(hdc, region, brush, width, width);
    let _ = DeleteObject(brush.into());
    let _ = DeleteObject(region.into());
}

unsafe fn text(hdc: HDC, label: &str, rect: RECT, size: i32, colour: COLORREF, bold: bool) {
    text_formatted(
        hdc,
        label,
        rect,
        size,
        colour,
        bold,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
}

unsafe fn text_formatted(
    hdc: HDC,
    label: &str,
    rect: RECT,
    size: i32,
    colour: COLORREF,
    bold: bool,
    format: DRAW_TEXT_FORMAT,
) {
    let wide: Vec<u16> = label.encode_utf16().collect();
    let font: HFONT = CreateFontW(
        -size.abs(),
        0,
        0,
        0,
        if bold {
            FW_BOLD.0 as i32
        } else {
            FW_NORMAL.0 as i32
        },
        0,
        0,
        0,
        DEFAULT_CHARSET,
        FONT_OUTPUT_PRECISION(0),
        FONT_CLIP_PRECISION(0),
        CLEARTYPE_QUALITY,
        DEFAULT_PITCH.0 as u32,
        w!("Segoe UI"),
    );
    let old = SelectObject(hdc, font.into());
    let _ = SetBkMode(hdc, TRANSPARENT);
    let _ = SetTextColor(hdc, colour);
    let mut r = rect;
    let mut wide_nul = wide;
    wide_nul.push(0);
    let _ = windows::Win32::Graphics::Gdi::DrawTextW(hdc, &mut wide_nul, &mut r, format);
    let _ = SelectObject(hdc, old);
    let _ = DeleteObject(font.into());
}

unsafe fn sprite(hdc: HDC, source: &Sprite, dest: RECT) {
    let blend = BLENDFUNCTION {
        // AC_SRC_OVER is 0; AC_SRC_ALPHA (1) belongs in AlphaFormat.
        BlendOp: 0,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let _ = AlphaBlend(
        hdc,
        dest.left,
        dest.top,
        dest.right - dest.left,
        dest.bottom - dest.top,
        source.dc,
        0,
        0,
        256,
        256,
        blend,
    );
}

unsafe fn paint(hwnd: HWND, state: &PickerState, hdc: HDC) {
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    let l = layout(width, height);
    let ivory = RGB(247, 244, 235);
    let ink = RGB(25, 29, 28);
    let muted = RGB(106, 111, 103);
    let lime = RGB(181, 235, 67);
    let pale_lime = RGB(227, 242, 191);
    let card = RGB(255, 253, 247);
    let shadow = RGB(218, 215, 205);
    let background_brush = windows::Win32::Graphics::Gdi::CreateSolidBrush(ivory);
    let _ = FillRect(hdc, &client, background_brush);
    let _ = DeleteObject(background_brush.into());
    fill_round(
        hdc,
        RECT {
            left: 8,
            top: 8,
            right: width - 8,
            bottom: height - 8,
        },
        ivory,
        24,
    );
    text(
        hdc,
        "CHOOSE YOUR MOTE",
        RECT {
            left: 32,
            top: 30,
            right: width - 360,
            bottom: 68,
        },
        30,
        ink,
        true,
    );
    text(
        hdc,
        "A tiny life for your desktop",
        RECT {
            left: 34,
            top: 73,
            right: width - 300,
            bottom: 98,
        },
        15,
        muted,
        false,
    );
    text(
        hdc,
        "ESC  CLOSE",
        RECT {
            left: width - 340,
            top: 38,
            right: width - 244,
            bottom: 72,
        },
        11,
        muted,
        true,
    );
    text(
        hdc,
        "×",
        RECT {
            left: width - 64,
            top: 30,
            right: width - 30,
            bottom: 72,
        },
        24,
        ink,
        true,
    );
    fill_round(
        hdc,
        l.quiet_toggle,
        if state.quiet { lime } else { card },
        12,
    );
    frame_round(
        hdc,
        l.quiet_toggle,
        if state.focus == FocusTarget::Quiet || state.quiet {
            ink
        } else {
            shadow
        },
        12,
        if state.focus == FocusTarget::Quiet {
            2
        } else {
            1
        },
    );
    text(
        hdc,
        if state.quiet {
            "QUIET  ON"
        } else {
            "QUIET  OFF"
        },
        RECT {
            left: l.quiet_toggle.left + 12,
            top: l.quiet_toggle.top,
            right: l.quiet_toggle.right - 8,
            bottom: l.quiet_toggle.bottom,
        },
        10,
        ink,
        true,
    );
    fill_round(
        hdc,
        l.motion_toggle,
        if state.reduced_motion { lime } else { card },
        12,
    );
    frame_round(
        hdc,
        l.motion_toggle,
        if state.focus == FocusTarget::Motion || state.reduced_motion {
            ink
        } else {
            shadow
        },
        12,
        if state.focus == FocusTarget::Motion {
            2
        } else {
            1
        },
    );
    text(
        hdc,
        if state.reduced_motion {
            "REDUCE MOTION  ON"
        } else {
            "REDUCE MOTION  OFF"
        },
        RECT {
            left: l.motion_toggle.left + 12,
            top: l.motion_toggle.top,
            right: l.motion_toggle.right - 8,
            bottom: l.motion_toggle.bottom,
        },
        10,
        ink,
        true,
    );

    fill_round(hdc, l.preview, RGB(238, 235, 224), 24);
    frame_round(hdc, l.preview, RGB(225, 221, 208), 24, 1);
    let chosen = state.species[state.cursor];
    let preview_size = (l.preview.right - l.preview.left - 44)
        .min(l.preview.bottom - l.preview.top - 122)
        .max(96);
    let preview_center = (l.preview.left + l.preview.right) / 2;
    let mut sprite_dest = RECT {
        left: preview_center - preview_size / 2,
        top: l.preview.top + 42,
        right: preview_center + preview_size / 2,
        bottom: l.preview.top + 42 + preview_size,
    };
    // A restrained float gives the character-select screen a little life.
    // Reduce Motion freezes it in the neutral pose.
    if !state.reduced_motion {
        let bob = (state.phase.sin() * 3.0) as i32;
        sprite_dest.top += bob;
        sprite_dest.bottom += bob;
    }
    sprite(hdc, &state.sprites[state.cursor], sprite_dest);
    text(
        hdc,
        chosen.name(),
        RECT {
            left: l.preview.left + 26,
            top: l.preview.bottom - 78,
            right: l.preview.right - 26,
            bottom: l.preview.bottom - 50,
        },
        24,
        ink,
        true,
    );
    text_formatted(
        hdc,
        chosen.description(),
        RECT {
            left: l.preview.left + 26,
            top: l.preview.bottom - 47,
            right: l.preview.right - 26,
            bottom: l.preview.bottom - 13,
        },
        14,
        muted,
        false,
        DT_LEFT | DT_WORDBREAK,
    );

    text(
        hdc,
        "ROSTER",
        RECT {
            left: l.cards[0].left,
            top: 112,
            right: width - 32,
            bottom: 138,
        },
        14,
        muted,
        true,
    );
    for (i, rect) in l.cards.iter().enumerate() {
        let selected = state.selected(i);
        let active = state.focus == FocusTarget::Roster(i);
        let fill = if selected { pale_lime } else { card };
        fill_round(hdc, *rect, fill, 17);
        frame_round(
            hdc,
            *rect,
            if active { ink } else { shadow },
            17,
            if active { 2 } else { 1 },
        );
        let compact_card = rect.bottom - rect.top < 160;
        let thumb_size = if compact_card {
            (rect.bottom - rect.top - 58).max(48)
        } else {
            104
        };
        let thumb = RECT {
            left: rect.left + 8,
            top: rect.top + 8,
            right: (rect.left + 8 + thumb_size).min(rect.right - 8),
            bottom: (rect.top + 8 + thumb_size).min(rect.bottom - 8),
        };
        sprite(hdc, &state.sprites[i], thumb);
        let name_rect = if compact_card {
            RECT {
                left: rect.left + 10,
                top: rect.bottom - 50,
                right: rect.right - 10,
                bottom: rect.bottom - 28,
            }
        } else {
            RECT {
                left: rect.left + 10,
                top: rect.top + 112,
                right: rect.right - 10,
                bottom: rect.top + 140,
            }
        };
        text(hdc, state.species[i].name(), name_rect, 14, ink, true);
        let action_rect = if compact_card {
            RECT {
                left: rect.left + 10,
                top: rect.bottom - 25,
                right: rect.right - 10,
                bottom: rect.bottom - 6,
            }
        } else {
            RECT {
                left: rect.left + 10,
                top: rect.top + 145,
                right: rect.right - 10,
                bottom: rect.top + 169,
            }
        };
        text(
            hdc,
            if selected {
                "IN THE PACK"
            } else if state.pets.len() == MAX_PETS {
                "PACK FULL"
            } else {
                "ADD TO PACK"
            },
            action_rect,
            11,
            if selected { RGB(73, 105, 24) } else { muted },
            true,
        );
        if selected {
            fill_round(
                hdc,
                RECT {
                    left: rect.right - 34,
                    top: rect.top + 12,
                    right: rect.right - 12,
                    bottom: rect.top + 34,
                },
                lime,
                10,
            );
            text(
                hdc,
                "✓",
                RECT {
                    left: rect.right - 32,
                    top: rect.top + 9,
                    right: rect.right - 14,
                    bottom: rect.top + 34,
                },
                13,
                ink,
                true,
            );
        }
    }

    text(
        hdc,
        "YOUR PACK",
        RECT {
            left: 32,
            top: height - 122,
            right: 300,
            bottom: height - 99,
        },
        11,
        muted,
        true,
    );
    for (i, rect) in l.lineup.iter().enumerate() {
        let occupied = state.pets.get(i).copied();
        fill_round(
            hdc,
            *rect,
            if occupied.is_some() { pale_lime } else { card },
            13,
        );
        frame_round(
            hdc,
            *rect,
            if state.focus == FocusTarget::Lineup(i) {
                ink
            } else {
                shadow
            },
            13,
            if state.focus == FocusTarget::Lineup(i) {
                2
            } else {
                1
            },
        );
        text(
            hdc,
            &format!("{}", i + 1),
            RECT {
                left: rect.left + 8,
                top: rect.top + 7,
                right: rect.left + 26,
                bottom: rect.top + 27,
            },
            10,
            muted,
            true,
        );
        if let Some(pet) = occupied {
            if let Some(index) = state.species.iter().position(|&s| s == pet) {
                sprite(
                    hdc,
                    &state.sprites[index],
                    RECT {
                        left: rect.left + 22,
                        top: rect.top - 7,
                        right: rect.right - 6,
                        bottom: rect.bottom + 8,
                    },
                );
            }
        } else {
            text(
                hdc,
                "+",
                RECT {
                    left: rect.left + 26,
                    top: rect.top + 12,
                    right: rect.right - 8,
                    bottom: rect.bottom - 8,
                },
                23,
                shadow,
                false,
            );
        }
    }
    fill_round(
        hdc,
        l.bring_home,
        if state.pets.is_empty() {
            RGB(211, 214, 202)
        } else {
            lime
        },
        16,
    );
    frame_round(
        hdc,
        l.bring_home,
        if state.focus == FocusTarget::BringHome {
            ink
        } else {
            RGB(211, 214, 202)
        },
        16,
        if state.focus == FocusTarget::BringHome {
            2
        } else {
            1
        },
    );
    text(
        hdc,
        "BRING THEM HOME  →",
        RECT {
            left: l.bring_home.left + 18,
            top: l.bring_home.top + 4,
            right: l.bring_home.right - 12,
            bottom: l.bring_home.bottom - 4,
        },
        13,
        if state.pets.is_empty() { muted } else { ink },
        true,
    );
    if state.focus == FocusTarget::Cancel {
        frame_round(hdc, l.cancel, ink, 10, 2);
    }
    text(
        hdc,
        "UP TO 4 PETS",
        RECT {
            left: width - 150,
            top: height - 122,
            right: width - 32,
            bottom: height - 101,
        },
        10,
        muted,
        true,
    );
}

fn point_in(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

fn mouse_point(lp: LPARAM) -> (i32, i32) {
    ((lp.0 as i16) as i32, ((lp.0 >> 16) as i16) as i32)
}

fn target_at(l: Layout, x: i32, y: i32) -> Option<FocusTarget> {
    if point_in(l.quiet_toggle, x, y) {
        Some(FocusTarget::Quiet)
    } else if point_in(l.motion_toggle, x, y) {
        Some(FocusTarget::Motion)
    } else if let Some(index) = l.cards.iter().position(|r| point_in(*r, x, y)) {
        Some(FocusTarget::Roster(index))
    } else if let Some(index) = l.lineup.iter().position(|r| point_in(*r, x, y)) {
        Some(FocusTarget::Lineup(index))
    } else if point_in(l.bring_home, x, y) {
        Some(FocusTarget::BringHome)
    } else if point_in(l.cancel, x, y) || (x > l.cancel.right + 164 && y < 90) {
        Some(FocusTarget::Cancel)
    } else {
        None
    }
}

unsafe fn finish(hwnd: HWND, cancelled: bool) {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return;
    }
    let state = &*(ptr as *const PickerState);
    if cancelled {
        state.cancel(hwnd);
    } else {
        state.commit(hwnd);
    }
}

unsafe fn activate_target(hwnd: HWND, target: FocusTarget) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return LRESULT(0);
    }
    let state = &mut *(ptr as *mut PickerState);
    match target {
        FocusTarget::Roster(index) => {
            state.cursor = index;
            state.toggle_cursor();
        }
        FocusTarget::Quiet => state.quiet = !state.quiet,
        FocusTarget::Motion => {
            state.reduced_motion = !state.reduced_motion;
            let _ = SetTimer(
                Some(hwnd),
                PICKER_TIMER,
                if state.reduced_motion { 120 } else { 33 },
                None,
            );
        }
        FocusTarget::Lineup(index) => {
            if index < state.pets.len() {
                state.pets.remove(index);
            }
        }
        FocusTarget::BringHome => {
            if state.pets.is_empty() {
                return LRESULT(0);
            }
            finish(hwnd, false);
            return LRESULT(0);
        }
        FocusTarget::Cancel => {
            finish(hwnd, true);
            return LRESULT(0);
        }
    }
    let _ = InvalidateRect(Some(hwnd), None, false);
    LRESULT(0)
}

/// Window procedure for the picker.  It intentionally does not call into
/// `App`, so keyboard focus and modal presentation remain easy to audit.
pub unsafe extern "system" fn picker_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        #[repr(C)]
        struct CreateStruct {
            lp_create_params: *mut std::ffi::c_void,
            _rest: [usize; 10],
        }
        let cs = lparam.0 as *const CreateStruct;
        if !cs.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lp_create_params as isize);
        }
        // A region supplies native rounded corners even on systems where the
        // DWM corner preference is unavailable.
        let mut r = RECT::default();
        let _ = GetClientRect(hwnd, &mut r);
        let region = CreateRoundRectRgn(
            0,
            0,
            (r.right - r.left).max(720),
            (r.bottom - r.top).max(500),
            24,
            24,
        );
        let _ = SetWindowRgn(hwnd, Some(region), true);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let state = &mut *(ptr as *mut PickerState);
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            let width = (client.right - client.left).max(1);
            let height = (client.bottom - client.top).max(1);
            state.buffer.ensure(width, height);
            let buffer_dc = state.buffer.dc;
            paint(hwnd, state, buffer_dc);
            let _ = BitBlt(hdc, 0, 0, width, height, Some(buffer_dc), 0, 0, SRCCOPY);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_TIMER if wparam.0 == PICKER_TIMER => {
            let elapsed = state.last_tick.elapsed().as_secs_f32();
            state.last_tick = Instant::now();
            if !state.reduced_motion {
                state.phase = (state.phase + elapsed).rem_euclid(std::f32::consts::TAU);
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = mouse_point(lparam);
            let l = layout(state.width, state.height);
            let old = state.hot;
            let target = target_at(l, x, y);
            state.hot = match target {
                Some(FocusTarget::Roster(index)) => Some(index),
                _ => None,
            };
            if old != state.hot {
                if let Some(FocusTarget::Roster(index)) = target {
                    state.cursor = index;
                }
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = mouse_point(lparam);
            let l = layout(state.width, state.height);
            state.pointer_down = true;
            state.pressed = target_at(l, x, y);
            if let Some(FocusTarget::Roster(index)) = state.pressed {
                state.cursor = index;
            }
            if let Some(target) = state.pressed {
                state.focus = target;
            }
            let _ = SetCapture(hwnd);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (x, y) = mouse_point(lparam);
            let l = layout(state.width, state.height);
            let target = target_at(l, x, y);
            let activate = state.pointer_down && state.pressed.is_some() && state.pressed == target;
            state.pointer_down = false;
            state.pressed = None;
            let _ = ReleaseCapture();
            if activate {
                return activate_target(hwnd, target.expect("matching pressed target"));
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            match wparam.0 as u32 {
                code if code == VK_LEFT.0 as u32 => {
                    let index = match state.focus {
                        FocusTarget::Roster(index) => index,
                        _ => state.cursor,
                    };
                    state.cursor = index;
                    state.focus = FocusTarget::Roster(index);
                    state.move_cursor(-1, 0);
                }
                code if code == VK_RIGHT.0 as u32 => {
                    let index = match state.focus {
                        FocusTarget::Roster(index) => index,
                        _ => state.cursor,
                    };
                    state.cursor = index;
                    state.focus = FocusTarget::Roster(index);
                    state.move_cursor(1, 0);
                }
                code if code == VK_UP.0 as u32 => {
                    let index = match state.focus {
                        FocusTarget::Roster(index) => index,
                        _ => state.cursor,
                    };
                    state.cursor = index;
                    state.focus = FocusTarget::Roster(index);
                    state.move_cursor(0, -1);
                }
                code if code == VK_DOWN.0 as u32 => {
                    let index = match state.focus {
                        FocusTarget::Roster(index) => index,
                        _ => state.cursor,
                    };
                    state.cursor = index;
                    state.focus = FocusTarget::Roster(index);
                    state.move_cursor(0, 1);
                }
                code if code == VK_RETURN.0 as u32 || code == VK_SPACE.0 as u32 => {
                    return activate_target(hwnd, state.focus);
                }
                code if code == VK_BACK.0 as u32 => {
                    if let FocusTarget::Roster(index) = state.focus {
                        let species = state.species[index];
                        if let Some(pos) = state.pets.iter().position(|&p| p == species) {
                            state.pets.remove(pos);
                        }
                    }
                }
                code if code == VK_ESCAPE.0 as u32 => {
                    finish(hwnd, true);
                    return LRESULT(0);
                }
                code if code == VK_TAB.0 as u32 => {
                    let reverse = GetKeyState(VK_SHIFT.0 as i32) < 0;
                    let current = state.focus.focus_index();
                    state.focus = if reverse {
                        FocusTarget::from_focus_index((current + 13) % 14)
                    } else {
                        FocusTarget::from_focus_index((current + 1) % 14)
                    };
                    if let FocusTarget::Roster(index) = state.focus {
                        state.cursor = index;
                    }
                }
                _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_CLOSE => {
            finish(hwnd, true);
            LRESULT(0)
        }
        WM_SIZE => {
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            state.width = (r.right - r.left).max(720);
            state.height = (r.bottom - r.top).max(500);
            let region = CreateRoundRectRgn(0, 0, state.width, state.height, 24, 24);
            let _ = windows::Win32::Graphics::Gdi::SetWindowRgn(hwnd, Some(region), true);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let _ = LoadCursorW(None, IDC_ARROW);
            LRESULT(1)
        }
        WM_GETMINMAXINFO => {
            #[repr(C)]
            struct MinMaxInfo {
                reserved: POINT,
                max_size: POINT,
                max_position: POINT,
                min_track: POINT,
                max_track: POINT,
            }
            let info = &mut *(lparam.0 as *mut MinMaxInfo);
            info.min_track = POINT { x: 720, y: 500 };
            LRESULT(0)
        }
        WM_NCDESTROY => {
            if let Ok(mut open) = open_windows().lock() {
                open.remove(&(state.owner.0 as isize));
            }
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr as *mut PickerState));
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_to_four_unique_pets() {
        let all = SpeciesId::all();
        let state = PickerState::new(
            HWND::default(),
            vec![all[0], all[0], all[1], all[2], all[3], all[4]],
            false,
            false,
        );
        assert_eq!(state.pets.len(), 4);
        assert_eq!(state.pets[0], all[0]);
        assert_eq!(state.pets[3], all[3]);
    }

    #[test]
    fn layout_is_inside_reasonable_window() {
        for (width, height) in [(1040, 680), (900, 600), (720, 500)] {
            let l = layout(width, height);
            assert!(l
                .cards
                .iter()
                .all(|r| r.right <= width && r.bottom <= height));
            assert!(l.bring_home.left < l.bring_home.right);
            assert!(l.lineup.iter().all(|r| r.bottom <= height));
        }
    }
}
