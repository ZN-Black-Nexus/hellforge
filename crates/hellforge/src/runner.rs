//! The main loop: runs a [`Game`] at a fixed rate, either interactively on a
//! platform backend or headless (screenshots, text dumps, hashes, benchmarks).

use crate::args::{Args, HELP};
use crate::gfx::{FmtBuf, Gfx};
use crate::input::{Button, Input, Key, Mouse, MouseButton};
use crate::math::Vec2;
use crate::platform::{Buf, Driver, Host, Opts};
use crate::rng::Rng;
use crate::Game;
use alloc::vec::Vec;
use core::fmt::Write as _;
use core::mem::MaybeUninit;

// The platform host, for `log!` and saves. Set once at start-up; the host
// lives in the platform's entry function for the whole program.
static mut HOST: Option<*mut (dyn Host + 'static)> = None;

fn host() -> Option<&'static mut dyn Host> {
    // SAFETY: single-threaded; set before the game runs, valid until exit.
    unsafe { (*core::ptr::addr_of!(HOST)).map(|h| &mut *h) }
}

#[doc(hidden)]
pub fn __log(args: core::fmt::Arguments) {
    let mut b = FmtBuf::<1024>::new();
    let _ = b.write_fmt(args);
    let _ = b.write_str("\n");
    if let Some(h) = host() {
        h.err(b.as_str().as_bytes());
    }
}

/// Print a line to the log (standard error): `log!("x = {}", x)`.
/// In the terminal renderer, redirect it: `game --term 2>log.txt`.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => { $crate::runner::__log(format_args!($($arg)*)) };
}

/// What `Game::update` gets: input, time, random numbers, saving and quitting.
pub struct Ctx {
    pub(crate) input: Input,
    frame: u64,
    fps: u32,
    w: i32,
    h: i32,
    rng: Rng,
    quit: bool,
    capture: bool,
    title: &'static str,
}

impl Ctx {
    fn new(title: &'static str, w: i32, h: i32, fps: u32, seed: u64) -> Ctx {
        Ctx { input: Input::new(), frame: 0, fps, w, h, rng: Rng::new(seed), quit: false, capture: false, title }
    }

    // ---------------------------------------------------------------- buttons

    /// Is the button down right now?
    pub fn held(&self, b: Button) -> bool {
        self.input.held(b)
    }
    /// Did the button go down this update? (true once per press)
    pub fn pressed(&self, b: Button) -> bool {
        self.input.pressed(b)
    }
    /// Did the button come up this update?
    pub fn released(&self, b: Button) -> bool {
        self.input.released(b)
    }
    /// -1 while Left is held, 1 for Right, 0 for neither (or both).
    pub fn axis_x(&self) -> i32 {
        self.held(Button::Right) as i32 - self.held(Button::Left) as i32
    }
    /// -1 while Up is held, 1 for Down, 0 for neither (or both).
    pub fn axis_y(&self) -> i32 {
        self.held(Button::Down) as i32 - self.held(Button::Up) as i32
    }
    /// Direction from the arrow buttons, each part -1, 0 or 1. Use
    /// `.normalized()` for the same speed diagonally.
    pub fn axis(&self) -> Vec2 {
        Vec2::new(self.axis_x() as f32, self.axis_y() as f32)
    }
    /// Did any key, button or mouse button go down this update?
    pub fn any_pressed(&self) -> bool {
        self.input.any_pressed()
    }

    // ---------------------------------------------------------------- keys and mouse

    pub fn key_held(&self, k: Key) -> bool {
        self.input.key_held(k)
    }
    pub fn key_pressed(&self, k: Key) -> bool {
        self.input.key_pressed(k)
    }
    pub fn key_released(&self, k: Key) -> bool {
        self.input.key_released(k)
    }
    /// Pointer position (screen pixels), movement and wheel for this update.
    pub fn mouse(&self) -> Mouse {
        self.input.mouse
    }
    pub fn mouse_held(&self, b: MouseButton) -> bool {
        self.input.mouse_held(b)
    }
    pub fn mouse_pressed(&self, b: MouseButton) -> bool {
        self.input.mouse_pressed(b)
    }
    pub fn mouse_released(&self, b: MouseButton) -> bool {
        self.input.mouse_released(b)
    }
    /// Hide and lock the pointer; `mouse().dx/dy` then report movement (mouse look).
    pub fn capture_mouse(&mut self, on: bool) {
        self.capture = on;
    }

    // ---------------------------------------------------------------- time and screen

    /// Updates since the game started (0 during the first update).
    pub fn frame(&self) -> u64 {
        self.frame
    }
    /// Seconds of game time since the start.
    pub fn time(&self) -> f32 {
        self.frame as f32 / self.fps as f32
    }
    /// Updates per second (`Game::FPS`).
    pub fn fps(&self) -> u32 {
        self.fps
    }
    /// Seconds per update (1 / FPS), for speeds in units per second.
    pub fn dt(&self) -> f32 {
        1.0 / self.fps as f32
    }
    pub fn width(&self) -> i32 {
        self.w
    }
    pub fn height(&self) -> i32 {
        self.h
    }

    // ---------------------------------------------------------------- random numbers

    /// A random `u32`.
    pub fn rand(&mut self) -> u32 {
        self.rng.next_u32()
    }
    /// A random integer in `lo..hi` (`hi` not included).
    pub fn rand_range(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.range(lo, hi)
    }
    /// A random `f32` in `0.0..1.0`.
    pub fn rand_float(&mut self) -> f32 {
        self.rng.float()
    }
    /// A random `f32` in `lo..hi`.
    pub fn rand_range_f(&mut self, lo: f32, hi: f32) -> f32 {
        self.rng.range_f(lo, hi)
    }
    /// True with probability `p` (0.0..=1.0).
    pub fn chance(&mut self, p: f32) -> bool {
        self.rng.chance(p)
    }
    /// The random generator itself (`pick`, `shuffle`, ...).
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    // ---------------------------------------------------------------- program

    /// End the game after this update.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    fn save_path(&self, slot: u32, out: &mut Buf<512>) -> bool {
        let Some(h) = host() else { return false };
        let mut name: Buf<64> = Buf::new();
        crate::platform::folder_name(self.title, &mut name);
        if !h.data_dir(name.as_bytes(), out) {
            return false;
        }
        out.push(&[h.sep()]).push(b"save").num(slot as u64).push(b".dat");
        true
    }

    /// Store `data` in save slot `slot` (a small file in the user's data
    /// folder). Returns false if it couldn't be written.
    pub fn save(&mut self, slot: u32, data: &[u8]) -> bool {
        let mut p: Buf<512> = Buf::new();
        if !self.save_path(slot, &mut p) {
            return false;
        }
        let Some(h) = host() else { return false };
        let Some(f) = h.create(p.as_bytes()) else { return false };
        let ok = h.write(f, data);
        h.close(f);
        ok
    }

    /// Read save slot `slot` into `buf`; returns the number of bytes read
    /// (0 if there is no save).
    pub fn load(&mut self, slot: u32, buf: &mut [u8]) -> usize {
        let mut p: Buf<512> = Buf::new();
        if !self.save_path(slot, &mut p) {
            return 0;
        }
        let Some(h) = host() else { return 0 };
        let Some(f) = h.open(p.as_bytes()) else { return 0 };
        let mut n = 0;
        while n < buf.len() {
            let k = h.read(f, &mut buf[n..]);
            if k == 0 {
                break;
            }
            n += k;
        }
        h.close(f);
        n
    }

    /// Read a whole save slot, or `None` if there is no save.
    pub fn load_vec(&mut self, slot: u32) -> Option<Vec<u8>> {
        let mut p: Buf<512> = Buf::new();
        if !self.save_path(slot, &mut p) {
            return None;
        }
        let h = host()?;
        let f = h.open(p.as_bytes())?;
        let mut v = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let k = h.read(f, &mut chunk);
            if k == 0 {
                break;
            }
            v.extend_from_slice(&chunk[..k]);
        }
        h.close(f);
        Some(v)
    }
}

// ==================================================================== driver

struct Runner<G: Game + 'static> {
    ctx: Ctx,
    gfx: Gfx,
    game: &'static mut G,
}

impl<G: Game> Driver for Runner<G> {
    fn size(&self) -> (usize, usize) {
        (G::WIDTH as usize, G::HEIGHT as usize)
    }
    fn title(&self) -> &'static str {
        G::TITLE
    }
    fn fps(&self) -> u32 {
        G::FPS
    }
    fn update(&mut self) {
        self.ctx.input.begin_update();
        if G::QUIT_ON_ESCAPE && self.ctx.input.key_pressed(Key::ESCAPE) {
            self.ctx.quit = true;
        }
        self.game.update(&mut self.ctx);
        self.ctx.frame += 1;
    }
    fn render(&mut self) {
        self.gfx.frame = self.ctx.frame;
        self.gfx.begin();
        self.game.draw(&mut self.gfx);
    }
    fn pixels(&self) -> &[u8] {
        self.gfx.pixels()
    }
    fn palette(&self) -> &[u8; 768] {
        self.gfx.palette_rgb()
    }
    fn key(&mut self, k: Key, down: bool) {
        self.ctx.input.key(k, down);
    }
    fn mouse_pos(&mut self, x: i32, y: i32) {
        self.ctx.input.mouse_pos(x, y);
    }
    fn mouse_motion(&mut self, dx: i32, dy: i32) {
        self.ctx.input.mouse_motion(dx, dy);
    }
    fn mouse_button(&mut self, b: MouseButton, down: bool) {
        self.ctx.input.mouse_button(b, down);
    }
    fn mouse_wheel(&mut self, dy: i32) {
        self.ctx.input.mouse_wheel(dy);
    }
    fn release_all(&mut self) {
        self.ctx.input.release_all();
    }
    fn quit_requested(&self) -> bool {
        self.ctx.quit
    }
    fn wants_capture(&self) -> bool {
        self.ctx.capture
    }
}

// ==================================================================== scripted input

#[derive(Default)]
struct Step {
    keys: Vec<Key>,
    mouse: Option<(i32, i32)>,
    buttons: Vec<MouseButton>,
    frames: u32,
}

fn parse_script(s: &str) -> Result<Vec<Step>, alloc::string::String> {
    let mut steps: Vec<Step> = Vec::new();
    for tok in s.split(|c: char| c.is_whitespace() || c == ';').filter(|t| !t.is_empty()) {
        let (items, frames) = match tok.rsplit_once(':') {
            Some((a, n)) if !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) => {
                (a, n.parse::<u32>().map_err(|_| alloc::format!("bad frame count in `{tok}`"))?)
            }
            _ => (tok, 1),
        };
        let mut st = Step { frames, ..Default::default() };
        for item in items.split('+') {
            let item = item.to_ascii_lowercase();
            if item == "wait" || item == "_" {
                continue;
            }
            if let Some(b) = Button::from_name(&item) {
                st.keys.push(b.keys()[0]);
            } else if let Some(k) = item.strip_prefix("key.") {
                st.keys.push(Key::from_name(k).ok_or_else(|| alloc::format!("unknown key `{k}` in `{tok}`"))?);
            } else if let Some((what, at)) = item.split_once('@') {
                let (x, y) = at.split_once(',').ok_or_else(|| alloc::format!("expected X,Y after @ in `{tok}`"))?;
                let num = |v: &str| v.trim().parse::<i32>().map_err(|_| alloc::format!("bad coordinate in `{tok}`"));
                st.mouse = Some((num(x)?, num(y)?));
                match what {
                    "click" => st.buttons.push(MouseButton::Left),
                    "rclick" => st.buttons.push(MouseButton::Right),
                    "mclick" => st.buttons.push(MouseButton::Middle),
                    "mouse" => {}
                    _ => return Err(alloc::format!("unknown mouse action `{what}` in `{tok}`")),
                }
            } else {
                return Err(alloc::format!(
                    "unknown input `{item}` in `{tok}` (buttons: up down left right a b x y start select; keys: key.NAME; mouse: click@X,Y)"
                ));
            }
        }
        // The same key in two steps in a row: let it go for one update in
        // between, so it counts as two presses.
        if let Some(prev) = steps.last() {
            if st.keys.iter().any(|k| prev.keys.contains(k)) || (!st.buttons.is_empty() && !prev.buttons.is_empty()) {
                steps.push(Step { frames: 1, ..Default::default() });
            }
        }
        steps.push(st);
    }
    Ok(steps)
}

// ==================================================================== headless output

fn write_png<G: Game>(host: &mut dyn Host, r: &Runner<G>, path: &[u8], scale: usize) -> bool {
    let Some(f) = host.create(path) else { return false };
    let (w, h) = (G::WIDTH as usize, G::HEIGHT as usize);
    let (px, pal) = (r.gfx.pixels(), r.gfx.palette_rgb());
    let mut buf = [0u8; 4096];
    let mut len = 0usize;
    let mut ok = true;
    crate::png::write_png(
        w * scale,
        h * scale,
        &mut |x, y| {
            let c = px[(y / scale) * w + x / scale] as usize;
            [pal[c * 3], pal[c * 3 + 1], pal[c * 3 + 2]]
        },
        &mut |bytes| {
            for &b in bytes {
                if len == buf.len() {
                    ok &= host.write(f, &buf);
                    len = 0;
                }
                buf[len] = b;
                len += 1;
            }
        },
    );
    ok &= host.write(f, &buf[..len]);
    host.close(f);
    ok
}

/// `{}` in a --shot name becomes the frame number (or it goes before `.png`).
fn numbered(path: &[u8], frame: u64, out: &mut Buf<512>) {
    if let Some(i) = path.windows(2).position(|w| w == b"{}") {
        out.push(&path[..i]).num(frame).push(&path[i + 2..]);
    } else if path.ends_with(b".png") {
        out.push(&path[..path.len() - 4]).push(b"-").num(frame).push(b".png");
    } else {
        out.push(path).push(b"-").num(frame);
    }
}

/// The frame as text: one hex digit per cell for the nearest base colour.
/// The most common colour of the whole frame is the background, printed as
/// `.`; a cell shows another colour if that covers a fifth of it, so thin
/// text and small sprites stay visible.
fn ascii<G: Game>(r: &Runner<G>, cols: u32, out: &mut dyn FnMut(&[u8])) {
    let (w, h) = (G::WIDTH as usize, G::HEIGHT as usize);
    let cols = (cols as usize).clamp(8, w);
    let cell_w = w as f32 / cols as f32;
    let cell_h = cell_w * 2.0; // text cells are about twice as tall as wide
    let rows = ((h as f32 / cell_h) as usize).max(1);
    let (px, pal) = (r.gfx.pixels(), r.gfx.palette_rgb());
    let mut base = [0u8; 256];
    for (i, b) in base.iter_mut().enumerate() {
        let c = ((pal[i * 3] as u32) << 16) | ((pal[i * 3 + 1] as u32) << 8) | pal[i * 3 + 2] as u32;
        let mut best = (i32::MAX, 0u8);
        for j in 0..16u8 {
            let d = crate::color::distance(crate::color::PALETTE[j as usize], c);
            if d < best.0 {
                best = (d, j);
            }
        }
        *b = best.1;
    }
    let mut total = [0u32; 16];
    for &p in px.iter() {
        total[base[p as usize] as usize] += 1;
    }
    let bg = (0..16).max_by_key(|&c| total[c]).unwrap_or(0);
    let mut head: Buf<200> = Buf::new();
    head.push(b"frame ").num(r.ctx.frame).push(b": ").num(w as u64).push(b"x").num(h as u64);
    head.push(b" pixels as ").num(cols as u64).push(b"x").num(rows as u64).push(b" cells. '.' = background (colour ");
    head.num(bg as u64).push(b"), 0-f = other base colours\n");
    out(head.as_bytes());
    let mut line: Vec<u8> = Vec::with_capacity(cols + 1);
    for row in 0..rows {
        line.clear();
        let y0 = (row as f32 * cell_h) as usize;
        let y1 = (((row + 1) as f32 * cell_h) as usize).min(h).max(y0 + 1);
        for col in 0..cols {
            let x0 = (col as f32 * cell_w) as usize;
            let x1 = (((col + 1) as f32 * cell_w) as usize).min(w).max(x0 + 1);
            let mut count = [0u32; 16];
            for y in y0..y1 {
                for x in x0..x1 {
                    count[base[px[y * w + x] as usize] as usize] += 1;
                }
            }
            let cell = ((y1 - y0) * (x1 - x0)) as u32;
            let other = (0..16).filter(|&c| c != bg).max_by_key(|&c| count[c]).unwrap_or(bg);
            let pick = if count[other] * 5 >= cell && count[other] > 0 { other } else { bg };
            line.push(if pick == bg { b'.' } else { b"0123456789abcdef"[pick] });
        }
        line.push(b'\n');
        out(&line);
    }
}

fn frame_hash(px: &[u8], pal: &[u8; 768]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in px {
        let i = b as usize * 3;
        for v in [pal[i], pal[i + 1], pal[i + 2]] {
            h = (h ^ v as u32).wrapping_mul(0x0100_0193);
        }
    }
    h
}

fn headless<G: Game>(r: &mut Runner<G>, host: &mut dyn Host, a: &Args) -> i32 {
    let script = match a.input.map(core::str::from_utf8) {
        None => Vec::new(),
        Some(Ok(s)) => match parse_script(s) {
            Ok(v) => v,
            Err(e) => {
                host.err(b"--input: ");
                host.err(e.as_bytes());
                host.err(b"\n");
                return 2;
            }
        },
        Some(Err(_)) => {
            host.err(b"--input: not valid UTF-8\n");
            return 2;
        }
    };
    let script_len: u32 = script.iter().map(|s| s.frames).sum();
    let total = a.frames.unwrap_or(script_len.max(1));
    let t0 = host.now_us();
    let (mut si, mut left) = (0usize, 0u32);
    let mut held: (Vec<Key>, Vec<MouseButton>) = (Vec::new(), Vec::new());
    let mut ran = 0u32;
    for f in 0..total {
        // advance the script
        while left == 0 && si < script.len() {
            for &k in &held.0 {
                r.key(k, false);
            }
            for &b in &held.1 {
                r.mouse_button(b, false);
            }
            let st = &script[si];
            for &k in &st.keys {
                r.key(k, true);
            }
            if let Some((x, y)) = st.mouse {
                r.mouse_pos(x, y);
            }
            for &b in &st.buttons {
                r.mouse_button(b, true);
            }
            held = (st.keys.clone(), st.buttons.clone());
            left = st.frames;
            si += 1;
        }
        if left == 0 && (!held.0.is_empty() || !held.1.is_empty()) {
            for &k in &held.0 {
                r.key(k, false);
            }
            for &b in &held.1 {
                r.mouse_button(b, false);
            }
            held = (Vec::new(), Vec::new());
        }
        left = left.saturating_sub(1);
        r.update();
        if a.bench {
            r.render(); // measure drawing too
        }
        ran = f + 1;
        if a.every > 0 && (f + 1) % a.every == 0 {
            if let Some(path) = a.shot {
                r.render();
                let mut p: Buf<512> = Buf::new();
                numbered(path, (f + 1) as u64, &mut p);
                if !write_png(host, r, p.as_bytes(), if a.scale > 0 { a.scale as usize } else { 2 }) {
                    host.err(b"could not write ");
                    host.err(p.as_bytes());
                    host.err(b"\n");
                    return 1;
                }
            }
        }
        if r.ctx.quit {
            break;
        }
    }
    r.render();
    let dt = host.now_us() - t0;

    if let Some(path) = a.shot {
        if a.every == 0 && !write_png(host, r, path, if a.scale > 0 { a.scale as usize } else { 2 }) {
            host.err(b"could not write ");
            host.err(path);
            host.err(b"\n");
            return 1;
        }
    }
    if let Some(cols) = a.ascii {
        ascii(r, cols, &mut |b| host.out(b));
    }
    if a.hash {
        let mut b: Buf<64> = Buf::new();
        b.push(b"frames ").num(ran as u64).push(b" hash ").hex(frame_hash(r.gfx.pixels(), r.gfx.palette_rgb())).push(b"\n");
        host.out(b.as_bytes());
    }
    if a.debug {
        let mut s = alloc::string::String::new();
        r.game.debug(&mut s);
        if !s.ends_with('\n') {
            s.push('\n');
        }
        host.out(s.as_bytes());
    }
    if a.bench {
        #[cfg(not(test))]
        let (used, peak, size) = crate::HEAP.stats();
        #[cfg(test)]
        let (used, peak, size) = (0, 0, 0);
        let mut b: Buf<256> = Buf::new();
        b.push(b"frames ").num(ran as u64).push(b"  total ").num(dt / 1000).push(b" ms  per update+draw ");
        b.num(dt / ran.max(1) as u64).push(b" us  peak RSS ").num(host.peak_rss_kb()).push(b" KiB  heap ");
        b.num(used as u64 / 1024).push(b"/").num(peak as u64 / 1024).push(b"/").num(size as u64 / 1024);
        b.push(b" KiB (now/peak/size)\n");
        host.out(b.as_bytes());
    }
    0
}

// ==================================================================== entry

/// Start a game. Called by the code `hellforge::main!` generates.
///
/// # Safety
/// Call once, with buffers that nothing else uses.
#[doc(hidden)]
pub unsafe fn run<G: Game>(
    host: &mut dyn Host,
    screen: &'static mut [u8],
    heap: &'static mut [u8],
    slot: &'static mut MaybeUninit<G>,
) -> i32 {
    #[cfg(not(test))]
    unsafe {
        crate::HEAP.init(heap.as_mut_ptr(), heap.len())
    };
    #[cfg(test)]
    let _ = heap;
    // SAFETY: the host outlives everything that uses this pointer.
    unsafe {
        let p: *mut (dyn Host + '_) = host;
        *core::ptr::addr_of_mut!(HOST) = Some(core::mem::transmute::<*mut (dyn Host + '_), *mut (dyn Host + 'static)>(p));
    }
    let a = Args::parse(&|i| host.arg(i));
    if a.help {
        host.out(G::TITLE.as_bytes());
        host.out(b" - made with Hellforge\n\n");
        host.out(HELP.as_bytes());
        return 0;
    }
    if a.version {
        host.out(G::TITLE.as_bytes());
        host.out(b" (Hellforge ");
        host.out(env!("CARGO_PKG_VERSION").as_bytes());
        host.out(b")\n");
        return 0;
    }
    if let Some(bad) = a.bad {
        host.err(b"unknown or incomplete option: ");
        host.err(bad);
        host.err(b" (see --help)\n");
        return 2;
    }
    let quiet = a.headless();
    let seed = a.seed.unwrap_or_else(|| if quiet { 1 } else { host.now_us().wrapping_mul(0x9e37_79b9) });
    let mut ctx = Ctx::new(G::TITLE, G::WIDTH as i32, G::HEIGHT as i32, G::FPS.max(1), seed);
    let mut gfx = Gfx::new(screen, G::WIDTH as usize, G::HEIGHT as usize);
    gfx.fps = G::FPS.max(1);
    let game = slot.write(G::new(&mut ctx));
    let mut r = Runner { ctx, gfx, game };
    if quiet {
        return headless(&mut r, host, &a);
    }
    let opts = Opts {
        backend: a.backend,
        scale: a.scale,
        fullscreen: a.fullscreen,
        nomouse: a.nomouse,
        colors: a.colors,
        term_fps: a.term_fps,
    };
    host.play(&mut r, &opts)
}
