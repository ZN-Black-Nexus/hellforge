//! # Hellforge
//!
//! A tiny, pure-Rust game engine for super-lightweight games. Every game is
//! one static binary per platform (Linux on any CPU without libc, Windows,
//! macOS), needs no data files and runs in a few hundred KB of RAM: in a
//! window, on the Linux framebuffer or in any terminal.
//!
//! A game is one type implementing [`Game`]:
//!
//! ```ignore
//! #![no_std]
//! #![no_main]
//! use hellforge::prelude::*;
//!
//! struct Hello {
//!     x: f32,
//! }
//!
//! impl Game for Hello {
//!     const TITLE: &'static str = "Hello";
//!
//!     fn new(_ctx: &mut Ctx) -> Self {
//!         Hello { x: 20.0 }
//!     }
//!
//!     fn update(&mut self, ctx: &mut Ctx) {
//!         self.x += ctx.axis_x() as f32 * 2.0;
//!     }
//!
//!     fn draw(&mut self, g: &mut Gfx) {
//!         g.clear(DARK_BLUE);
//!         g.fill_rect(self.x, 80, 8, 8, YELLOW);
//!         g.text(4, 4, "Arrow keys move", WHITE);
//!     }
//! }
//!
//! hellforge::main!(Hello);
//! ```
//!
//! See `AGENTS.md` in the repository for the full guide.

#![cfg_attr(not(test), no_std)]
#![cfg_attr(
    any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "sparc",
        target_arch = "sparc64",
        target_arch = "m68k",
        target_arch = "csky",
        target_arch = "hexagon"
    ),
    feature(asm_experimental_arch)
)]
#![allow(clippy::too_many_arguments, clippy::new_without_default)]

extern crate alloc;
// memcpy and friends (there is no libc anywhere).
#[cfg(not(test))]
extern crate hellforge_rt;

#[doc(hidden)]
pub mod args;
pub mod color;
pub mod font;
pub mod gfx;
#[doc(hidden)]
pub mod heap;
pub mod input;
pub mod math;
#[doc(hidden)]
pub mod platform;
pub mod png;
pub mod ray;
pub mod rng;
#[doc(hidden)]
pub mod runner;
pub mod sprite;
pub mod tilemap;

pub use gfx::Gfx;
pub use input::{Button, Key, Mouse, MouseButton};
pub use runner::Ctx;

/// A game. Implement this for your game's state and pass the type to
/// [`main!`](crate::main!).
pub trait Game: Sized + 'static {
    /// Window title; also names the folder saves go in.
    const TITLE: &'static str = "Hellforge game";
    /// Screen width in pixels. You always draw at this size; the window
    /// scales it up with square pixels and black bars as needed.
    const WIDTH: u32 = 320;
    /// Screen height in pixels.
    const HEIGHT: u32 = 180;
    /// Updates per second: `update` runs exactly this often.
    const FPS: u32 = 60;
    /// Quit when Escape is pressed. Set to false to use Escape yourself.
    const QUIT_ON_ESCAPE: bool = true;

    /// Create the game state (runs once, before the first update).
    fn new(ctx: &mut Ctx) -> Self;
    /// Advance the game by one step: read input from `ctx`, move things.
    fn update(&mut self, ctx: &mut Ctx);
    /// Draw the current state. Don't change game state here.
    fn draw(&mut self, g: &mut Gfx);
    /// Optional: describe the state as text for `--debug` (tests, AI agents).
    fn debug(&self, out: &mut alloc::string::String) {
        let _ = out;
    }
}

#[cfg(not(test))]
#[global_allocator]
pub(crate) static HEAP: heap::Heap = heap::Heap::new();

/// Everything a game needs: `use hellforge::prelude::*;`
pub mod prelude {
    pub use crate::color::*;
    pub use crate::font::{CHAR_H, CHAR_W, LINE_H};
    pub use crate::input::{Button, Key, Mouse, MouseButton};
    pub use crate::math::{Float, Num, Rect, Vec2, approach, isqrt, lerp, rect, vec2, wrap, wrapi};
    pub use crate::ray::{Billboard, Camera3d, Wall};
    pub use crate::rng::Rng;
    pub use crate::sprite::{Sprite, anim};
    pub use crate::tilemap::{Grid, Hit, RayHit, Tilemap};
    pub use crate::{Ctx, Game, Gfx};
    pub use crate::{log, sprite, text, tilemap};
    pub use alloc::boxed::Box;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
    pub use alloc::{format, vec};
    pub use core::f32::consts::{PI, TAU};
    pub use core::fmt::Write as _;
}

/// Turn a [`Game`] type into a program: `hellforge::main!(MyGame);`
///
/// The game's memory for `Vec`, `String`, `Box` and `format!` defaults to
/// 1 MiB; choose another size with `hellforge::main!(MyGame, heap = 4 * 1024 * 1024);`
/// (untouched heap costs no RAM on desktop systems).
#[macro_export]
macro_rules! main {
    ($game:ty) => {
        $crate::main!($game, heap = 1024 * 1024);
    };
    ($game:ty, heap = $heap:expr $(,)?) => {
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub fn __hellforge_entry(host: &mut dyn $crate::platform::Host) -> i32 {
            const __W: usize = <$game as $crate::Game>::WIDTH as usize;
            const __H: usize = <$game as $crate::Game>::HEIGHT as usize;
            const _: () = assert!(
                __W >= 1 && __H >= 1 && __W <= 4096 && __H <= 4096,
                "Game::WIDTH and Game::HEIGHT must be between 1 and 4096"
            );
            static mut __SCREEN: [u8; __W * __H] = [0; __W * __H];
            static mut __HEAP: [u8; $heap] = [0; $heap];
            static mut __GAME: ::core::mem::MaybeUninit<$game> = ::core::mem::MaybeUninit::uninit();
            // SAFETY: the platform entry point calls this exactly once.
            unsafe {
                $crate::runner::run::<$game>(
                    host,
                    &mut *::core::ptr::addr_of_mut!(__SCREEN),
                    &mut *::core::ptr::addr_of_mut!(__HEAP),
                    &mut *::core::ptr::addr_of_mut!(__GAME),
                )
            }
        }
    };
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    let mut b = gfx::FmtBuf::<1024>::new();
    let _ = write!(b, "panic: {}", info.message());
    if let Some(l) = info.location() {
        let _ = write!(b, " (at {}:{}:{})", l.file(), l.line(), l.column());
    }
    if b.as_str().starts_with("panic: memory allocation of") {
        let _ = write!(b, "\nthe game ran out of heap: give it more with hellforge::main!(MyGame, heap = 8 * 1024 * 1024)");
    }
    let _ = b.write_str("\n");
    platform::panic_exit(b.as_str().as_bytes())
}

// The prebuilt `alloc` library is compiled for unwinding and references
// these; games abort on panic instead, so they are never called.
#[cfg(all(not(test), not(target_os = "windows")))]
#[doc(hidden)]
#[unsafe(no_mangle)]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

#[cfg(all(not(test), not(target_os = "windows")))]
#[doc(hidden)]
#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}
