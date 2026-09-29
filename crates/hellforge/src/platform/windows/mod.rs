//! Windows platform layer: no C runtime, Win32 only (Windows 7 and newer;
//! the console renderer needs Windows 10 for VT sequences).

pub mod sys;
pub mod win;

use crate::input::Key;
use crate::platform::tui::{Colors, Tui};
use crate::platform::{Backend, Buf, Clock, Driver, Host, Opts};
use sys::*;

const KEY_BACKSPACE: u16 = Key::BACKSPACE.0;
const KEY_TAB: u16 = Key::TAB.0;
const KEY_ENTER: u16 = Key::ENTER.0;
const KEY_ESCAPE: u16 = Key::ESCAPE.0;
const KEY_SPACE: u16 = Key::SPACE.0;
const KEY_UP: u16 = Key::UP.0;
const KEY_DOWN: u16 = Key::DOWN.0;
const KEY_LEFT: u16 = Key::LEFT.0;
const KEY_RIGHT: u16 = Key::RIGHT.0;
const KEY_SHIFT: u16 = Key::SHIFT.0;
const KEY_CTRL: u16 = Key::CTRL.0;
const KEY_ALT: u16 = Key::ALT.0;
const KEY_F1: u16 = Key::F1.0;
const KEY_PGUP: u16 = Key::PAGE_UP.0;
const KEY_PGDN: u16 = Key::PAGE_DOWN.0;
const KEY_HOME: u16 = Key::HOME.0;
const KEY_END: u16 = Key::END.0;
const KEY_INSERT: u16 = Key::INSERT.0;
const KEY_DELETE: u16 = Key::DELETE.0;
const KEY_PAUSE: u16 = Key::PAUSE.0;

// ------------------------------------------------------------------ runtime symbols

/// MSVC-style code expects this when floating point appears anywhere.
#[unsafe(no_mangle)]
pub static _fltused: i32 = 0;

/// Stack probe for frames larger than a page (x86_64 MSVC ABI: size in rax,
/// touch each page, don't move rsp).
#[cfg(target_arch = "x86_64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn __chkstk() {
    core::arch::naked_asm!(
        "push rcx",
        "push rax",
        "lea rcx, [rsp + 24]",
        "cmp rax, 0x1000",
        "jb 2f",
        "3:",
        "sub rcx, 0x1000",
        "test [rcx], rcx",
        "sub rax, 0x1000",
        "cmp rax, 0x1000",
        "ja 3b",
        "2:",
        "sub rcx, rax",
        "test [rcx], rcx",
        "pop rax",
        "pop rcx",
        "ret",
    )
}

/// 32-bit x86 MSVC stack probe: size in eax; this one also moves esp.
#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _chkstk() {
    core::arch::naked_asm!(
        "push ecx",
        "lea ecx, [esp + 8]",
        "cmp eax, 0x1000",
        "jb 2f",
        "3:",
        "sub ecx, 0x1000",
        "test [ecx], eax",
        "sub eax, 0x1000",
        "cmp eax, 0x1000",
        "jae 3b",
        "2:",
        "sub ecx, eax",
        "test [ecx], eax",
        "mov eax, esp",
        "mov esp, ecx",
        "mov ecx, [eax]",
        "mov eax, [eax + 4]",
        "jmp eax",
    )
}

// 32-bit x86 MSVC code calls these for 64-bit arithmetic (normally in the
// C runtime). Arguments sit on the stack and the callee pops them; we
// re-push them for compiler_builtins' portable versions.
#[cfg(target_arch = "x86")]
unsafe extern "C" {
    fn __udivdi3(n: u64, d: u64) -> u64;
    fn __umoddi3(n: u64, d: u64) -> u64;
    fn __divdi3(n: i64, d: i64) -> i64;
    fn __moddi3(n: i64, d: i64) -> i64;
    fn __muldi3(a: i64, b: i64) -> i64;
}

#[cfg(target_arch = "x86")]
macro_rules! msvc_i64_helper {
    ($name:ident, $target:ident) => {
        #[unsafe(no_mangle)]
        #[unsafe(naked)]
        pub unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "push dword ptr [esp + 16]",
                "push dword ptr [esp + 16]",
                "push dword ptr [esp + 16]",
                "push dword ptr [esp + 16]",
                "call {f}",
                "add esp, 16",
                "ret 16",
                f = sym $target,
            )
        }
    };
}

#[cfg(target_arch = "x86")]
msvc_i64_helper!(_aulldiv, __udivdi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_aullrem, __umoddi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_alldiv, __divdi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_allrem, __moddi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_allmul, __muldi3);

/// ARM64 Windows stack probe: size/16 in x15, touch each page, keep registers.
#[cfg(target_arch = "aarch64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn __chkstk() {
    core::arch::naked_asm!(
        "lsl x16, x15, #4",
        "mov x17, sp",
        "1:",
        "sub x17, x17, #4096",
        "subs x16, x16, #4096",
        "ldr xzr, [x17]",
        "b.gt 1b",
        "ret",
    )
}

// ------------------------------------------------------------------ console output

static mut STDOUT: HANDLE = 0;
static mut STDERR: HANDLE = 0;

/// Standard output/error: inherited handles (pipes, files), or the console
/// of the program that started us.
fn std_handle(err: bool) -> HANDLE {
    unsafe {
        let slot = if err { &mut *core::ptr::addr_of_mut!(STDERR) } else { &mut *core::ptr::addr_of_mut!(STDOUT) };
        if *slot == 0 {
            let which = if err { 0xffff_fff4 } else { 0xffff_fff5 }; // STD_ERROR / STD_OUTPUT
            let mut h = GetStdHandle(which);
            if h == 0 || h == INVALID_HANDLE {
                AttachConsole(0xffff_ffff); // ATTACH_PARENT_PROCESS
                h = GetStdHandle(which);
            }
            *slot = if h == 0 { INVALID_HANDLE } else { h };
        }
        *slot
    }
}

pub fn console_out(b: &[u8]) {
    let h = std_handle(false);
    if h != INVALID_HANDLE {
        stdout_write(h, b);
    }
}

pub fn console_err(b: &[u8]) {
    let h = std_handle(true);
    if h != INVALID_HANDLE {
        stdout_write(h, b);
    }
}

fn message_box(msg: &[u8]) {
    let mut w = [0u16; 1100];
    let mut t = [0u16; 16];
    unsafe {
        MessageBoxW(0, wide(msg, &mut w).as_ptr(), wide(b"Hellforge", &mut t).as_ptr(), 0x10);
    }
}

pub fn fatal(msg: &[u8]) -> ! {
    message_box(msg);
    unsafe { ExitProcess(1) }
}

/// Panic: print to the console if there is one, otherwise show a message box.
pub fn panic_exit(msg: &[u8]) -> ! {
    let h = std_handle(true);
    if h != INVALID_HANDLE {
        stdout_write(h, msg);
    } else {
        message_box(msg);
    }
    unsafe { ExitProcess(101) }
}

// ------------------------------------------------------------------ command line

static mut ARGBUF: [u8; 8192] = [0; 8192];
static mut ARGS: [(u16, u16); 64] = [(0, 0); 64];
static mut ARGC: usize = 0;

/// Split the command line (UTF-16) into UTF-8 arguments, Windows-style quoting.
fn parse_cmdline() {
    unsafe {
        let mut p = GetCommandLineW();
        let buf = &mut *core::ptr::addr_of_mut!(ARGBUF);
        let args = &mut *core::ptr::addr_of_mut!(ARGS);
        let mut len = 0usize;
        let mut n = 0usize;
        loop {
            while *p == b' ' as u16 || *p == b'\t' as u16 {
                p = p.add(1);
            }
            if *p == 0 || n == args.len() {
                break;
            }
            let start = len;
            let mut quoted = false;
            while *p != 0 && (quoted || (*p != b' ' as u16 && *p != b'\t' as u16)) {
                if *p == b'"' as u16 {
                    quoted = !quoted;
                    p = p.add(1);
                    continue;
                }
                // one UTF-16 code point (with its surrogate pair) -> UTF-8
                let pair = [*p, *p.add(1)];
                let units = if (0xd800..0xdc00).contains(&pair[0]) && (0xdc00..0xe000).contains(&pair[1]) { 2 } else { 1 };
                let ch = core::char::decode_utf16(pair[..units].iter().copied()).next().and_then(|r| r.ok()).unwrap_or('?');
                let mut tmp = [0u8; 4];
                let e = ch.encode_utf8(&mut tmp).as_bytes();
                if len + e.len() <= buf.len() {
                    buf[len..len + e.len()].copy_from_slice(e);
                    len += e.len();
                }
                p = p.add(units);
            }
            args[n] = (start as u16, len as u16);
            n += 1;
        }
        ARGC = n;
    }
}

fn arg(i: usize) -> Option<&'static [u8]> {
    unsafe {
        if i >= ARGC {
            return None;
        }
        let (a, b) = (*core::ptr::addr_of!(ARGS))[i];
        let buf: &'static [u8; 8192] = &*core::ptr::addr_of!(ARGBUF);
        Some(&buf[a as usize..b as usize])
    }
}

// ------------------------------------------------------------------ host

pub struct WinHost;

impl Host for WinHost {
    fn arg(&self, i: usize) -> Option<&'static [u8]> {
        arg(i)
    }
    fn out(&mut self, b: &[u8]) {
        console_out(b);
    }
    fn err(&mut self, b: &[u8]) {
        console_err(b);
    }
    fn now_us(&mut self) -> u64 {
        now_us()
    }
    fn sleep_us(&mut self, us: u64) {
        sleep_us(us)
    }
    fn create(&mut self, path: &[u8]) -> Option<isize> {
        let mut w = [0u16; 520];
        // GENERIC_WRITE, CREATE_ALWAYS
        let h = unsafe { CreateFileW(wide(path, &mut w).as_ptr(), 0x4000_0000, 0, core::ptr::null(), 2, 0x80, 0) };
        if h == INVALID_HANDLE { None } else { Some(h) }
    }
    fn open(&mut self, path: &[u8]) -> Option<isize> {
        let mut w = [0u16; 520];
        // GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING
        let h = unsafe { CreateFileW(wide(path, &mut w).as_ptr(), 0x8000_0000, 1, core::ptr::null(), 3, 0x80, 0) };
        if h == INVALID_HANDLE { None } else { Some(h) }
    }
    fn write(&mut self, f: isize, data: &[u8]) -> bool {
        stdout_write(f, data)
    }
    fn read(&mut self, f: isize, buf: &mut [u8]) -> usize {
        let mut n = 0u32;
        let ok = unsafe { ReadFile(f, buf.as_mut_ptr(), buf.len().min(1 << 30) as u32, &mut n, core::ptr::null_mut()) };
        if ok == 0 { 0 } else { n as usize }
    }
    fn close(&mut self, f: isize) {
        unsafe {
            CloseHandle(f);
        }
    }
    fn data_dir(&mut self, game: &[u8], out: &mut Buf<512>) -> bool {
        let mut name = [0u16; 16];
        let mut val = [0u16; 300];
        let n = unsafe { GetEnvironmentVariableW(wide(b"APPDATA", &mut name).as_ptr(), val.as_mut_ptr(), 299) } as usize;
        if n > 0 && n < 299 {
            narrow(&val[..n], out);
            out.push(b"\\");
        }
        out.push(game);
        let mut w = [0u16; 520];
        unsafe {
            CreateDirectoryW(wide(out.as_bytes(), &mut w).as_ptr(), core::ptr::null());
        }
        true
    }
    fn sep(&self) -> u8 {
        b'\\'
    }
    fn peak_rss_kb(&mut self) -> u64 {
        // PROCESS_MEMORY_COUNTERS: cb, PageFaultCount, PeakWorkingSetSize, ...
        let mut c = [0usize; 10];
        let cb = core::mem::size_of_val(&c) as u32;
        c[0] = cb as usize;
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), c.as_mut_ptr(), cb) };
        if ok == 0 {
            return 0;
        }
        // On 64-bit the two u32s share word 0; on 32-bit they are words 0 and 1.
        let peak = if core::mem::size_of::<usize>() == 8 { c[1] } else { c[2] };
        (peak / 1024) as u64
    }
    fn play(&mut self, d: &mut dyn Driver, o: &Opts) -> i32 {
        if o.backend == Backend::Term {
            return console_run(d, o);
        }
        win::run(d, o)
    }
}

// ------------------------------------------------------------------ entry

#[unsafe(no_mangle)]
pub extern "system" fn WinMainCRTStartup() -> ! {
    parse_cmdline();
    unsafe {
        timeBeginPeriod(1);
    }
    let code = crate::platform::enter(&mut WinHost);
    unsafe { ExitProcess(code as u32) }
}

#[unsafe(no_mangle)]
pub extern "system" fn mainCRTStartup() -> ! {
    WinMainCRTStartup()
}

// ------------------------------------------------------------------ console mode

static mut TUI: Tui = Tui::new();

fn vk_to_key(vk: u16, ch: u16) -> u16 {
    match vk {
        0x25 => KEY_LEFT,
        0x26 => KEY_UP,
        0x27 => KEY_RIGHT,
        0x28 => KEY_DOWN,
        0x0d => KEY_ENTER,
        0x1b => KEY_ESCAPE,
        0x09 => KEY_TAB,
        0x08 => KEY_BACKSPACE,
        0x20 => KEY_SPACE,
        0x10 | 0xa0 | 0xa1 => KEY_SHIFT,
        0x11 | 0xa2 | 0xa3 => KEY_CTRL,
        0x12 | 0xa4 | 0xa5 => KEY_ALT,
        0x70..=0x7b => KEY_F1 + (vk - 0x70),
        0x13 => KEY_PAUSE,
        0x21 => KEY_PGUP,
        0x22 => KEY_PGDN,
        0x23 => KEY_END,
        0x24 => KEY_HOME,
        0x2d => KEY_INSERT,
        0x2e => KEY_DELETE,
        0x30..=0x39 => vk,
        0x41..=0x5a => vk + 32,
        0xbd | 0x6d => b'-' as u16,
        0xbb | 0x6b => b'=' as u16,
        0xbc => b',' as u16,
        0xbe => b'.' as u16,
        0xbf => b'/' as u16,
        _ => {
            if (32..127).contains(&ch) {
                (ch as u8).to_ascii_lowercase() as u16
            } else {
                0
            }
        }
    }
}

pub fn vk_key(vk: u16) -> u16 {
    vk_to_key(vk, 0)
}

fn console_run(d: &mut dyn Driver, o: &Opts) -> i32 {
    unsafe {
        AttachConsole(0xffff_ffff);
        let out = GetStdHandle(0xffff_fff5);
        let inp = GetStdHandle(0xffff_fff6);
        STDOUT = out;
        let mut mode = 0u32;
        GetConsoleMode(out, &mut mode);
        // ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN
        if SetConsoleMode(out, mode | 0x1 | 0x4 | 0x8) == 0 {
            fatal(b"This console can't show VT graphics (needs Windows 10 or newer). Run without --term.");
        }
        SetConsoleOutputCP(65001);
        let mut imode = 0u32;
        GetConsoleMode(inp, &mut imode);
        // Raw keys: no line input / echo / processed input.
        SetConsoleMode(inp, 0x80);
        let tui = &mut *core::ptr::addr_of_mut!(TUI);
        tui.colors = if o.colors != 0 { Colors::detect(None, None, o.colors) } else { Colors::True };
        let size = |tui: &mut Tui| {
            let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
            if GetConsoleScreenBufferInfo(out, &mut info) != 0 {
                tui.cols = (info.srWindow[2] - info.srWindow[0] + 1).max(16) as usize;
                tui.rows = (info.srWindow[3] - info.srWindow[1] + 1).max(8) as usize;
            }
        };
        size(tui);
        stdout_write(out, Tui::ENTER);
        let mut clock = Clock::new(d.fps(), now_us());
        let frame_us: u64 = 1_000_000 / o.term_fps.max(1) as u64;
        let (mut last_size, mut last_frame) = (0u64, 0u64);
        let mut recs: [INPUT_RECORD; 32] = core::mem::zeroed();
        let mut writer = |b: &[u8]| {
            stdout_write(out, b);
        };
        loop {
            let mut n = 0u32;
            GetNumberOfConsoleInputEvents(inp, &mut n);
            while n > 0 {
                let mut got = 0u32;
                ReadConsoleInputW(inp, recs.as_mut_ptr(), 32, &mut got);
                for r in &recs[..got as usize] {
                    if r.EventType == 1 {
                        let k = &r.Event;
                        let key = vk_to_key(k.wVirtualKeyCode, k.uChar);
                        if key == b'c' as u16 && k.dwControlKeyState & 0xc != 0 && k.bKeyDown != 0 {
                            stdout_write(out, Tui::LEAVE);
                            SetConsoleMode(inp, imode);
                            return 0;
                        }
                        if key != 0 {
                            d.key(Key(key), k.bKeyDown != 0);
                        }
                    }
                }
                GetNumberOfConsoleInputEvents(inp, &mut n);
            }
            if d.quit_requested() {
                break;
            }
            let now = now_us();
            let steps = clock.due(now);
            for _ in 0..steps {
                d.update();
            }
            if now - last_size > 500_000 {
                last_size = now;
                let (c, r) = (tui.cols, tui.rows);
                size(tui);
                if c != tui.cols || r != tui.rows {
                    tui.force = true;
                }
            }
            if steps > 0 && now - last_frame >= frame_us {
                last_frame = now;
                d.render();
                let (sw, sh) = d.size();
                tui.frame(d.pixels(), sw, sh, d.palette(), &mut writer);
            }
            let wait = clock.wait(now_us());
            if wait > 1000 {
                sleep_us(wait.min(10_000));
            }
        }
        stdout_write(out, Tui::LEAVE);
        SetConsoleMode(inp, imode);
    }
    0
}
