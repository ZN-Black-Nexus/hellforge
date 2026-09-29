//! Terminal backend: plays in any terminal (local, SSH, serial console).

use super::nr;
use super::*;
use crate::input::Key;
use crate::platform::tui::{Colors, HeldKeys, KEY_SHIFT, KeyDecoder, Tui};
use crate::platform::{Clock, Driver, Opts};

static mut SAVED_TERMIOS: [u8; 64] = [0; 64];
static mut RAW_ACTIVE: bool = false;
static mut TUI: Tui = Tui::new();

fn termios_get(buf: &mut [u8; 64]) -> bool {
    ioctl(0, nr::TCGETS, buf.as_mut_ptr() as usize) == 0
}

fn termios_set(buf: &[u8; 64]) -> bool {
    ioctl(0, nr::TCSETS, buf.as_ptr() as usize) == 0
}

fn get_u32(b: &[u8; 64], off: usize) -> u32 {
    u32::from_ne_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn set_u32(b: &mut [u8; 64], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_ne_bytes());
}

/// Switch stdin to raw, non-blocking mode. Returns false if it isn't a terminal.
pub fn raw_mode() -> bool {
    let mut t = [0u8; 64];
    if !termios_get(&mut t) {
        return false;
    }
    unsafe {
        *core::ptr::addr_of_mut!(SAVED_TERMIOS) = t;
    }
    let iflag = get_u32(&t, 0) & !(nr::termios::IXON | nr::ICRNL | nr::BRKINT | nr::INPCK | nr::ISTRIP);
    let lflag = get_u32(&t, 12) & !(nr::termios::ICANON | nr::termios::ECHO | nr::termios::ISIG | nr::termios::IEXTEN);
    set_u32(&mut t, 0, iflag);
    set_u32(&mut t, 12, lflag);
    t[nr::termios::CC_OFFSET + nr::termios::VMIN] = 0;
    t[nr::termios::CC_OFFSET + nr::termios::VTIME] = 0;
    if !termios_set(&t) {
        return false;
    }
    unsafe {
        RAW_ACTIVE = true;
    }
    true
}

/// Restore the terminal. Safe to call from the panic handler.
pub fn emergency_restore() {
    unsafe {
        if RAW_ACTIVE {
            let t = *core::ptr::addr_of!(SAVED_TERMIOS);
            termios_set(&t);
            write_all(1, Tui::LEAVE);
            RAW_ACTIVE = false;
        }
    }
}

fn winsize() -> (usize, usize) {
    let mut ws = [0u16; 4];
    if ioctl(1, nr::TIOCGWINSZ, ws.as_mut_ptr() as usize) == 0 && ws[0] > 0 && ws[1] > 0 {
        (ws[1] as usize, ws[0] as usize)
    } else {
        (80, 24)
    }
}

pub fn run(d: &mut dyn Driver, env: &Env, o: &Opts) -> i32 {
    if !raw_mode() {
        write_all(2, b"stdin is not a terminal (use --window or --fb, or run it in a terminal)\n");
        return 1;
    }
    // SAFETY: single-threaded; only this function uses the renderer state.
    let tui = unsafe { &mut *core::ptr::addr_of_mut!(TUI) };
    tui.colors = Colors::detect(env.var(b"TERM"), env.var(b"COLORTERM"), o.colors);
    let (cols, rows) = winsize();
    tui.cols = cols;
    tui.rows = rows;
    write_all(1, Tui::ENTER);
    write_all(1, Tui::KITTY_QUERY);

    let mut dec = KeyDecoder::new();
    let mut held = HeldKeys::new();
    let mut kitty_on = false;
    let frame_us: u64 = 1_000_000 / o.term_fps.max(1) as u64;
    let mut clock = Clock::new(d.fps(), now_us());
    let (mut last_frame, mut last_size_check) = (0u64, 0u64);
    let mut dirty = true;
    let mut inbuf = [0u8; 256];
    let mut writer = |b: &[u8]| {
        write_all(1, b);
    };
    loop {
        // ---- input
        loop {
            let n = read(0, &mut inbuf);
            if n <= 0 {
                break;
            }
            let now = now_us() / 1000;
            dec.feed(&inbuf[..n as usize], &mut |ev| {
                if ev.key == 0 {
                    return;
                }
                if kitty_on {
                    if ev.shift && ev.down {
                        d.key(Key(KEY_SHIFT), true);
                    }
                    d.key(Key(ev.key), ev.down);
                    return;
                }
                // No key-up events: synthesise them from the auto-repeat stream.
                if ev.shift && held.press(KEY_SHIFT, now) {
                    d.key(Key(KEY_SHIFT), true);
                }
                if held.press(ev.key, now) {
                    d.key(Key(ev.key), true);
                }
            });
            if dec.quit {
                break;
            }
        }
        if dec.kitty && !kitty_on {
            // The terminal can report key releases: switch that on.
            kitty_on = true;
            write_all(1, Tui::KITTY_ON);
        }
        if !kitty_on {
            held.expire(now_us() / 1000, &mut |k| d.key(Key(k), false));
        }
        if dec.quit || d.quit_requested() {
            break;
        }

        // ---- fixed-rate updates
        let now = now_us();
        let steps = clock.due(now);
        for _ in 0..steps {
            d.update();
            if d.quit_requested() {
                break;
            }
        }

        // ---- terminal resized?
        if now - last_size_check > 500_000 {
            last_size_check = now;
            let (c, r) = winsize();
            if c != tui.cols || r != tui.rows {
                tui.cols = c;
                tui.rows = r;
                tui.force = true;
                dirty = true;
            }
        }

        // ---- draw (capped: terminals are slow)
        if (steps > 0 || dirty) && now - last_frame >= frame_us {
            last_frame = now;
            dirty = false;
            d.render();
            let (sw, sh) = d.size();
            tui.frame(d.pixels(), sw, sh, d.palette(), &mut writer);
        }
        let wait = clock.wait(now_us());
        if wait > 0 {
            sleep_us(wait.min(10_000));
        }
    }
    emergency_restore();
    0
}
