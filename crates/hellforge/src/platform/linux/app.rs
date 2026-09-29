//! Linux host: files, clock, output and choosing a display backend.

use super::nr;
use super::*;
use crate::platform::{Backend, Driver, Host, Opts};

pub struct LinuxHost {
    pub env: Env,
}

impl Host for LinuxHost {
    fn arg(&self, i: usize) -> Option<&'static [u8]> {
        self.env.arg(i)
    }
    fn out(&mut self, b: &[u8]) {
        write_all(1, b);
    }
    fn err(&mut self, b: &[u8]) {
        write_all(2, b);
    }
    fn now_us(&mut self) -> u64 {
        now_us()
    }
    fn sleep_us(&mut self, us: u64) {
        sleep_us(us)
    }
    fn create(&mut self, path: &[u8]) -> Option<isize> {
        let mut p: Buf<512> = Buf::new();
        p.push(path);
        let fd = open(p.cstr(), nr::O_WRONLY | nr::O_CREAT | nr::O_TRUNC, 0o644);
        if fd < 0 { None } else { Some(fd as isize) }
    }
    fn open(&mut self, path: &[u8]) -> Option<isize> {
        let mut p: Buf<512> = Buf::new();
        p.push(path);
        let fd = open(p.cstr(), nr::O_RDONLY, 0);
        if fd < 0 { None } else { Some(fd as isize) }
    }
    fn write(&mut self, f: isize, data: &[u8]) -> bool {
        write_all(f as i32, data)
    }
    fn read(&mut self, f: isize, buf: &mut [u8]) -> usize {
        let n = read(f as i32, buf);
        if n <= 0 { 0 } else { n as usize }
    }
    fn close(&mut self, f: isize) {
        close(f as i32);
    }
    fn data_dir(&mut self, game: &[u8], out: &mut Buf<512>) -> bool {
        if let Some(x) = self.env.var(b"XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            out.push(x).push(b"/").push(game);
        } else if let Some(h) = self.env.var(b"HOME").filter(|v| !v.is_empty()) {
            out.push(h).push(b"/.local/share/").push(game);
        } else {
            out.push(b".").push(game);
        }
        // mkdir -p, one component at a time
        let d = out.as_bytes();
        for i in 1..=d.len() {
            if i == d.len() || d[i] == b'/' {
                let mut p: Buf<512> = Buf::new();
                p.push(&d[..i]);
                mkdir(p.cstr(), 0o755);
            }
        }
        true
    }
    fn peak_rss_kb(&mut self) -> u64 {
        let fd = open(b"/proc/self/status\0", nr::O_RDONLY, 0);
        if fd < 0 {
            return 0;
        }
        let mut buf = [0u8; 2048];
        let n = read(fd, &mut buf);
        close(fd);
        if n <= 0 {
            return 0;
        }
        for line in buf[..n as usize].split(|&c| c == b'\n') {
            if let Some(rest) = line.strip_prefix(b"VmHWM:") {
                let mut v = 0u64;
                for &c in rest {
                    if c.is_ascii_digit() {
                        v = v * 10 + (c - b'0') as u64;
                    }
                }
                return v;
            }
        }
        0
    }
    fn play(&mut self, d: &mut dyn Driver, o: &Opts) -> i32 {
        let env = &self.env;
        let backend = match o.backend {
            Backend::Auto => {
                if env.var(b"DISPLAY").is_some_and(|v| !v.is_empty()) {
                    Backend::X11
                } else if super::fb::available() && !env.var(b"SSH_CONNECTION").is_some_and(|v| !v.is_empty()) {
                    Backend::Fb
                } else {
                    Backend::Term
                }
            }
            b => b,
        };
        let fallback = |what: &[u8], msg: &[u8]| {
            write_all(2, what);
            write_all(2, msg);
            write_all(2, b"; using the terminal instead\n");
        };
        match backend {
            Backend::X11 | Backend::Window => match super::x11::run(d, env, o) {
                Ok(c) => c,
                Err(msg) => {
                    fallback(b"X11: ", msg);
                    super::term::run(d, env, o)
                }
            },
            Backend::Fb => match super::fb::run(d, env, o) {
                Ok(c) => c,
                Err(msg) => {
                    fallback(b"framebuffer: ", msg);
                    super::term::run(d, env, o)
                }
            },
            _ => super::term::run(d, env, o),
        }
    }
}

pub fn main(env: Env) -> i32 {
    let mut host = LinuxHost { env };
    crate::platform::enter(&mut host)
}

/// Panic: put the console and terminal back, print, exit.
pub fn panic_exit(msg: &[u8]) -> ! {
    super::fb::restore_console();
    super::term::emergency_restore();
    write_all(2, msg);
    exit(101)
}
