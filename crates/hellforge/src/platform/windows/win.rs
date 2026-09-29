//! Win32/GDI window: the 8-bit screen stretched into the window with
//! StretchDIBits, square pixels, black bars, Alt+Enter for fullscreen.

use super::sys::*;
use super::{fatal, vk_key};
use crate::input::{Key, MouseButton};
use crate::platform::{Clock, Driver, Opts, fit, to_screen, window_scale};
use alloc::vec::Vec;

const WM_DESTROY: u32 = 0x0002;
const WM_SIZE: u32 = 0x0005;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_PAINT: u32 = 0x000f;
const WM_CLOSE: u32 = 0x0010;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_SETCURSOR: u32 = 0x0020;
const WM_INPUT: u32 = 0x00ff;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_MOUSEWHEEL: u32 = 0x020a;

const WS_OVERLAPPEDWINDOW: u32 = 0x00cf_0000;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const GWL_STYLE: i32 = -16;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_FRAMECHANGED: u32 = 0x0020;
const VK_RETURN: usize = 0x0d;

struct State {
    driver: Option<*mut (dyn Driver + 'static)>,
    quit: bool,
    grabbed: bool,
    focused: bool,
    dirty: bool,
    w: i32,
    h: i32,
    /// Borderless window covering the monitor.
    full: bool,
    /// Where the normal window goes when leaving fullscreen.
    windowed: RECT,
    sw: usize,
    sh: usize,
    place: (usize, usize, usize, usize),
}

static mut ST: State = State {
    driver: None,
    quit: false,
    grabbed: false,
    focused: true,
    dirty: true,
    w: 960,
    h: 540,
    full: false,
    windowed: RECT { left: 0, top: 0, right: 0, bottom: 0 },
    sw: 1,
    sh: 1,
    place: (0, 0, 1, 1),
};

static mut BMI: BITMAPINFO256 = BITMAPINFO256 {
    biSize: 40,
    biWidth: 1,
    biHeight: -1,
    biPlanes: 1,
    biBitCount: 8,
    biCompression: 0,
    biSizeImage: 0,
    biXPelsPerMeter: 0,
    biYPelsPerMeter: 0,
    biClrUsed: 256,
    biClrImportant: 256,
    colors: [0; 256],
};

fn st() -> &'static mut State {
    // SAFETY: the window procedure and main loop run on the same thread.
    unsafe { &mut *core::ptr::addr_of_mut!(ST) }
}

fn driver() -> Option<&'static mut dyn Driver> {
    // SAFETY: set by `run` for as long as the window exists.
    st().driver.map(|d| unsafe { &mut *d })
}

/// Alt+Enter: switch between the normal window and a borderless window
/// covering the monitor it is on.
fn toggle_fullscreen(hwnd: HWND) {
    let s = st();
    // SAFETY: plain Win32 calls on our own window.
    unsafe {
        let r = if s.full {
            SetWindowLongW(hwnd, GWL_STYLE, (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as i32);
            s.windowed
        } else {
            GetWindowRect(hwnd, &mut s.windowed);
            let mut mi = MONITORINFO { cbSize: core::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            GetMonitorInfoW(MonitorFromWindow(hwnd, 2), &mut mi); // nearest monitor
            SetWindowLongW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);
            mi.rcMonitor
        };
        SetWindowPos(hwnd, 0, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_FRAMECHANGED);
        s.full = !s.full;
        if s.grabbed {
            let mut w = RECT::default();
            GetWindowRect(hwnd, &mut w);
            ClipCursor(&w);
        }
    }
    s.dirty = true;
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    let s = st();
    match msg {
        WM_CLOSE | WM_DESTROY => {
            s.quit = true;
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_SIZE => {
            s.w = (lp & 0xffff) as i32;
            s.h = ((lp >> 16) & 0xffff) as i32;
            s.dirty = true;
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { core::mem::zeroed() };
            unsafe {
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps);
            }
            s.dirty = true;
            return 0;
        }
        WM_KILLFOCUS => {
            s.focused = false;
            if let Some(d) = driver() {
                d.release_all();
            }
        }
        WM_SETCURSOR => {
            if s.grabbed && (lp & 0xffff) == 1 {
                unsafe {
                    SetCursor(0);
                }
                return 1;
            }
        }
        WM_SYSKEYDOWN if wp == VK_RETURN && (lp & (1 << 29)) != 0 => {
            if (lp & (1 << 30)) == 0 {
                toggle_fullscreen(hwnd); // Alt+Enter (not its auto-repeat)
            }
            return 0;
        }
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let k = vk_key(wp as u16);
            if let Some(d) = driver() {
                if k != 0 {
                    d.key(Key(k), down);
                }
            }
            if down && k == Key::F4.0 && (lp & (1 << 29)) != 0 {
                s.quit = true; // Alt+F4
            }
            return 0; // swallow Alt/F10 so the system menu doesn't steal focus
        }
        WM_MOUSEMOVE => {
            if !s.grabbed {
                if let Some(d) = driver() {
                    let (x, y) = ((lp & 0xffff) as i16 as i32, ((lp >> 16) & 0xffff) as i16 as i32);
                    let (px, py) = to_screen(x, y, s.place, s.sw, s.sh);
                    d.mouse_pos(px, py);
                }
            }
            return 0;
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP => {
            s.focused = true;
            if let Some(d) = driver() {
                let (b, down) = match msg {
                    WM_LBUTTONDOWN => (MouseButton::Left, true),
                    WM_LBUTTONUP => (MouseButton::Left, false),
                    WM_RBUTTONDOWN => (MouseButton::Right, true),
                    WM_RBUTTONUP => (MouseButton::Right, false),
                    WM_MBUTTONDOWN => (MouseButton::Middle, true),
                    _ => (MouseButton::Middle, false),
                };
                d.mouse_button(b, down);
            }
            return 0;
        }
        WM_MOUSEWHEEL => {
            if let Some(d) = driver() {
                let delta = ((wp >> 16) & 0xffff) as i16;
                d.mouse_wheel(if delta > 0 { 1 } else { -1 });
            }
            return 0;
        }
        WM_INPUT => {
            let mut raw: RAWMOUSEINPUT = unsafe { core::mem::zeroed() };
            let mut size = core::mem::size_of::<RAWMOUSEINPUT>() as u32;
            let hdr = (2 * core::mem::size_of::<usize>() + 8) as u32;
            let got = unsafe { GetRawInputData(lp, 0x1000_0003, &mut raw as *mut _ as *mut u8, &mut size, hdr) };
            if got != u32::MAX && raw.dwType == 0 && s.grabbed && raw.usFlags & 1 == 0 {
                if let Some(d) = driver() {
                    // mouse counts -> roughly screen pixels
                    let (dw, dh) = (s.place.2.max(1) as i32, s.place.3.max(1) as i32);
                    d.mouse_motion(raw.lLastX * s.sw as i32 / dw, raw.lLastY * s.sh as i32 / dh);
                }
            }
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

pub fn run(d: &mut dyn Driver, o: &Opts) -> i32 {
    let s = st();
    let (sw, sh) = d.size();
    s.sw = sw;
    s.sh = sh;
    // SAFETY: the pointer is only used while this function runs.
    s.driver = Some(unsafe { core::mem::transmute::<*mut (dyn Driver + '_), *mut (dyn Driver + 'static)>(d as *mut dyn Driver) });
    let mut cls_name = [0u16; 16];
    let mut title = [0u16; 256];
    let cls = wide(b"HellforgeWnd", &mut cls_name).as_ptr();
    // 8-bit DIB rows must start on 4-byte boundaries.
    let stride = (sw + 3) & !3;
    let mut padded: Vec<u8> = if stride != sw { alloc::vec![0; stride * sh] } else { Vec::new() };
    unsafe {
        let inst = GetModuleHandleW(core::ptr::null());
        let wc = WNDCLASSEXW {
            cbSize: core::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 3, // CS_HREDRAW | CS_VREDRAW
            lpfnWndProc: wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: 0,
            hCursor: LoadCursorW(0, 32512 as *const u16), // IDC_ARROW
            hbrBackground: 0,
            lpszMenuName: core::ptr::null(),
            lpszClassName: cls,
            hIconSm: 0,
        };
        if RegisterClassExW(&wc) == 0 {
            fatal(b"Could not register the window class.");
        }
        let scale = window_scale(sw, sh, o.scale) as i32;
        let mut r = RECT { left: 0, top: 0, right: sw as i32 * scale, bottom: sh as i32 * scale };
        AdjustWindowRect(&mut r, WS_OVERLAPPEDWINDOW, 0);
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        let (mw, mh) = (GetSystemMetrics(0), GetSystemMetrics(1));
        let (style, x, y, w, h) = if o.fullscreen {
            // Alt+Enter later brings back a normal window in the middle of the screen.
            s.full = true;
            s.windowed = RECT { left: (mw - ww) / 2, top: (mh - wh) / 2, right: (mw + ww) / 2, bottom: (mh + wh) / 2 };
            (WS_POPUP | WS_VISIBLE, 0, 0, mw, mh)
        } else {
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE, 0x8000_0000u32 as i32, 0x8000_0000u32 as i32, ww, wh)
        };
        let hwnd = CreateWindowExW(0, cls, wide(d.title().as_bytes(), &mut title).as_ptr(), style, x, y, w, h, 0, 0, inst, core::ptr::null());
        if hwnd == 0 {
            fatal(b"Could not create the window.");
        }
        ShowWindow(hwnd, 1);
        let rid = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 2, dwFlags: 0, hwndTarget: hwnd };
        RegisterRawInputDevices(&rid, 1, core::mem::size_of::<RAWINPUTDEVICE>() as u32);

        let mut clock = Clock::new(d.fps(), now_us());
        let mut msg: MSG = core::mem::zeroed();
        let bmi = &mut *core::ptr::addr_of_mut!(BMI);
        bmi.biWidth = sw as i32;
        bmi.biHeight = -(sh as i32);
        while !s.quit && !d.quit_requested() {
            while PeekMessageW(&mut msg, 0, 0, 0, 1) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            s.focused = GetForegroundWindow() == hwnd;
            let want = d.wants_capture() && s.focused && !o.nomouse;
            if want != s.grabbed {
                s.grabbed = want;
                if want {
                    let mut r = RECT::default();
                    GetWindowRect(hwnd, &mut r);
                    ClipCursor(&r);
                    ShowCursor(0);
                } else {
                    ClipCursor(core::ptr::null());
                    ShowCursor(1);
                }
            }
            let steps = clock.due(now_us());
            for _ in 0..steps {
                d.update();
                if d.quit_requested() {
                    break;
                }
            }
            if steps > 0 || s.dirty {
                s.dirty = false;
                d.render();
                let pal = d.palette();
                for i in 0..256 {
                    bmi.colors[i] = (pal[i * 3] as u32) << 16 | (pal[i * 3 + 1] as u32) << 8 | pal[i * 3 + 2] as u32;
                }
                let mut px = d.pixels();
                if !padded.is_empty() {
                    for y in 0..sh {
                        padded[y * stride..y * stride + sw].copy_from_slice(&px[y * sw..(y + 1) * sw]);
                    }
                    px = &padded;
                }
                let mut rc = RECT::default();
                GetClientRect(hwnd, &mut rc);
                let (cw, ch) = (rc.right.max(1) as usize, rc.bottom.max(1) as usize);
                s.place = fit(sw, sh, cw, ch);
                let (ox, oy, dw, dh) = (s.place.0 as i32, s.place.1 as i32, s.place.2 as i32, s.place.3 as i32);
                let (cw, ch) = (cw as i32, ch as i32);
                let dc = GetDC(hwnd);
                // Black bars (BLACKNESS raster op).
                if ox > 0 {
                    PatBlt(dc, 0, 0, ox, ch, 0x42);
                    PatBlt(dc, ox + dw, 0, cw - ox - dw, ch, 0x42);
                }
                if oy > 0 {
                    PatBlt(dc, 0, 0, cw, oy, 0x42);
                    PatBlt(dc, 0, oy + dh, cw, ch - oy - dh, 0x42);
                }
                StretchDIBits(dc, ox, oy, dw, dh, 0, 0, sw as i32, sh as i32, px.as_ptr(), bmi, 0, 0x00cc_0020);
                ReleaseDC(hwnd, dc);
            }
            let wait = clock.wait(now_us());
            if wait > 1000 {
                sleep_us(wait.min(4_000));
            }
        }
        ClipCursor(core::ptr::null());
    }
    s.driver = None;
    0
}
