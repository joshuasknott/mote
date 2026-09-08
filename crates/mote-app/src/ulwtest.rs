//! ULW isolation test: solid red layered window + environment report.
//! Run: cargo run -p mote-app --bin ulwtest  (stays up 12 s)
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, POINT, SIZE};
use windows::Win32::Graphics::Dwm::DwmIsCompositionEnabled;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
use windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;
use windows::Win32::UI::WindowsAndMessaging::*;

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn main() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        // Environment report.
        let remote = GetSystemMetrics(SM_REMOTESESSION);
        let comp = DwmIsCompositionEnabled();
        println!("SM_REMOTESESSION={remote} DwmIsCompositionEnabled={comp:?}");

        let instance: HINSTANCE = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .unwrap()
            .into();
        let cls = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: w!("UlwTest"),
            ..Default::default()
        };
        assert_ne!(RegisterClassExW(&cls), 0);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("UlwTest"),
            w!("UlwTest"),
            WS_POPUP,
            1400,
            900,
            256,
            256,
            None,
            None,
            Some(instance),
            None,
        )
        .unwrap();

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: 256,
            biHeight: -256,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        let mem_dc = CreateCompatibleDC(None);
        println!("mem_dc valid={} dib_comparable...", !mem_dc.is_invalid());
        SelectObject(mem_dc, dib.into());
        // Solid opaque red.
        let px = std::slice::from_raw_parts_mut(bits as *mut u8, 256 * 256 * 4);
        for p in px.as_chunks_mut::<4>().0 {
            p[0] = 0;
            p[1] = 0;
            p[2] = 255;
            p[3] = 255;
        }
        let pt = POINT { x: 1400, y: 900 };
        let size = SIZE { cx: 256, cy: 256 };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: 0,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let r = UpdateLayeredWindow(
            hwnd,
            None,
            Some(&pt),
            Some(&size),
            Some(mem_dc),
            Some(&src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        println!("ULW result={r:?}");
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        println!("red square should be at (1400,900) for 12 s");
        let t0 = std::time::Instant::now();
        let mut msg = MSG::default();
        while t0.elapsed().as_secs() < 12 {
            if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        let _ = PCWSTR::null();
    }
}
