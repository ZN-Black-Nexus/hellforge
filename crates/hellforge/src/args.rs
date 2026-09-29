//! Command-line options every Hellforge game understands.

use crate::platform::{Backend, parse_u32};

pub struct Args {
    pub backend: Backend,
    pub scale: u32,
    pub fullscreen: bool,
    pub nomouse: bool,
    pub colors: u16,
    pub term_fps: u32,
    pub frames: Option<u32>,
    pub shot: Option<&'static [u8]>,
    pub every: u32,
    pub input: Option<&'static [u8]>,
    pub ascii: Option<u32>,
    pub hash: bool,
    pub bench: bool,
    pub debug: bool,
    pub seed: Option<u64>,
    pub help: bool,
    pub version: bool,
    /// An option we didn't understand, or one missing its value.
    pub bad: Option<&'static [u8]>,
}

fn parse_u64(s: &[u8]) -> Option<u64> {
    if s.is_empty() || s.len() > 19 {
        return None;
    }
    let mut v: u64 = 0;
    for &c in s {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v * 10 + (c - b'0') as u64;
    }
    Some(v)
}

impl Args {
    pub fn parse(get: &dyn Fn(usize) -> Option<&'static [u8]>) -> Args {
        let mut a = Args {
            backend: Backend::Auto,
            scale: 0,
            fullscreen: false,
            nomouse: false,
            colors: 0,
            term_fps: 30,
            frames: None,
            shot: None,
            every: 0,
            input: None,
            ascii: None,
            hash: false,
            bench: false,
            debug: false,
            seed: None,
            help: false,
            version: false,
            bad: None,
        };
        let mut i = 1;
        while let Some(arg) = get(i) {
            let val = get(i + 1);
            let num = val.and_then(parse_u32);
            let mut used = 1;
            let mut need = |ok: bool| {
                if ok {
                    used = 2;
                } else if a.bad.is_none() {
                    a.bad = Some(arg);
                }
            };
            match arg {
                b"--term" | b"-t" => a.backend = Backend::Term,
                b"--x11" => a.backend = Backend::X11,
                b"--fb" => a.backend = Backend::Fb,
                b"--window" => a.backend = Backend::Window,
                b"--fullscreen" | b"-f" => a.fullscreen = true,
                b"--nomouse" => a.nomouse = true,
                b"--scale" => {
                    need(num.is_some());
                    a.scale = num.unwrap_or(0).min(64);
                }
                b"--colors" => {
                    need(num.is_some());
                    a.colors = num.unwrap_or(0).min(999) as u16;
                }
                b"--fps" => {
                    need(num.is_some());
                    a.term_fps = num.unwrap_or(30).clamp(1, 1000);
                }
                b"--frames" => {
                    need(num.is_some());
                    a.frames = num;
                }
                b"--shot" => {
                    need(val.is_some());
                    a.shot = val;
                }
                b"--every" => {
                    need(num.is_some());
                    a.every = num.unwrap_or(0);
                }
                b"--input" => {
                    need(val.is_some());
                    a.input = val;
                }
                b"--ascii" => {
                    // optional width
                    if num.is_some() {
                        used = 2;
                    }
                    a.ascii = Some(num.unwrap_or(80));
                }
                b"--seed" => {
                    let s = val.and_then(parse_u64);
                    need(s.is_some());
                    a.seed = s;
                }
                b"--hash" => a.hash = true,
                b"--bench" => a.bench = true,
                b"--debug" => a.debug = true,
                b"--help" | b"-h" => a.help = true,
                b"--version" | b"-V" => a.version = true,
                _ => {
                    if a.bad.is_none() {
                        a.bad = Some(arg);
                    }
                }
            }
            i += used;
        }
        a
    }

    /// Run without a screen and print or save results instead.
    pub fn headless(&self) -> bool {
        self.frames.is_some()
            || self.shot.is_some()
            || self.ascii.is_some()
            || self.input.is_some()
            || self.hash
            || self.bench
            || self.debug
    }
}

pub const HELP: &str = "usage: GAME [options]

Play:
  --window, --x11     a window (the default on desktops)
  --term, -t          play in the terminal (works over SSH)
  --fb                Linux framebuffer + evdev (consoles, kiosks)
  --fullscreen, -f    start fullscreen (Alt+Enter switches)
  --scale N           window size multiplier
  --nomouse           never capture the mouse
  --colors N          terminal colours: 24, 256, 16 or 2
  --fps N             terminal frame rate cap (default 30)

Test without a screen (scripts, CI, AI agents):
  --frames N          run N updates, then stop
  --input \"STEPS\"     scripted input: steps separated by spaces, each
                      NAME[+NAME...][:FRAMES]; names are buttons (up down
                      left right a b x y start select), keys (key.q,
                      key.space, key.esc, key.f1, ...), mouse (click@X,Y
                      rclick@X,Y mouse@X,Y) or wait. e.g. \"right:30 a wait:10\"
  --shot FILE.png     save the last frame (2x size; --scale N to change);
                      with --every N, save every N updates ({} = frame number)
  --ascii [COLS]      print the last frame as text: one hex digit per cell
                      (the base colour 0-f), '.' for black
  --hash              print a hash of the last frame (to compare runs)
  --debug             print the game's Game::debug() text at the end
  --bench             print speed and memory use
  --seed N            random seed (headless runs use 1 unless set)

  --help, --version

Keys: arrows/WASD move, Z X C V are the A B X Y buttons, Enter = Start,
Tab = Select, Esc quits, Alt+Enter toggles fullscreen.
";
