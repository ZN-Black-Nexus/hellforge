//! Maths without the standard library.
//!
//! * [`Float`] gives `f32` the usual `sqrt`, `sin`, `cos`, `atan2`, `floor`,
//!   `powf`, ... methods (pure Rust, the same result on every CPU). It is in
//!   the prelude, so `x.sqrt()` just works.
//! * [`Vec2`] and [`Rect`] for positions, velocities and hit boxes.
//! * [`Num`] lets drawing functions take `i32` or `f32` coordinates alike.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub use core::f32::consts::{PI as PI32, TAU};

// ---------------------------------------------------------------- f32 helpers

#[inline]
fn fabs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7fff_ffff)
}

fn ffloor(x: f32) -> f32 {
    if !(fabs(x) < 8_388_608.0) {
        return x; // already whole (or NaN / infinite)
    }
    let t = x as i32 as f32;
    if t > x { t - 1.0 } else { t }
}

/// `x * 2^n`
fn ldexp(mut x: f32, mut n: i32) -> f32 {
    while n > 127 {
        x *= f32::from_bits(0x7f00_0000); // 2^127
        n -= 127;
    }
    while n < -126 {
        x *= f32::from_bits(0x0080_0000); // 2^-126
        n += 126;
    }
    x * f32::from_bits(((n + 127) as u32) << 23)
}

// Octant reduction constants (pi/4 split into three parts for precision).
const FOPI: f32 = 1.273_239_5;
const DP1: f32 = 0.785_156_25;
const DP2: f32 = 2.418_756_5e-4;
const DP3: f32 = 3.774_895e-8;

fn sin_poly(x: f32, z: f32) -> f32 {
    ((-1.951_529_6e-4 * z + 8.332_161e-3) * z - 1.666_665_5e-1) * z * x + x
}

fn cos_poly(z: f32) -> f32 {
    ((2.443_315_7e-5 * z - 1.388_731_6e-3) * z + 4.166_664_6e-2) * z * z - 0.5 * z + 1.0
}

/// Reduce |x| to [-pi/4, pi/4]; returns (reduced x, octant 0..3, flip sign).
fn reduce(ax: f32) -> (f32, u32, bool) {
    let mut j = (ax * FOPI) as u32;
    let mut y = j as f32;
    if j & 1 != 0 {
        j += 1;
        y += 1.0;
    }
    j &= 7;
    let mut flip = false;
    if j > 3 {
        flip = true;
        j -= 4;
    }
    (((ax - y * DP1) - y * DP2) - y * DP3, j, flip)
}

fn fsin(x: f32) -> f32 {
    if !x.is_finite() {
        return f32::NAN;
    }
    let (r, j, mut neg) = reduce(fabs(x));
    if x < 0.0 {
        neg = !neg;
    }
    let z = r * r;
    let y = if j == 1 || j == 2 { cos_poly(z) } else { sin_poly(r, z) };
    if neg { -y } else { y }
}

fn fcos(x: f32) -> f32 {
    if !x.is_finite() {
        return f32::NAN;
    }
    let (r, j, mut neg) = reduce(fabs(x));
    if j > 1 {
        neg = !neg;
    }
    let z = r * r;
    let y = if j == 1 || j == 2 { sin_poly(r, z) } else { cos_poly(z) };
    if neg { -y } else { y }
}

fn fatan(x: f32) -> f32 {
    let (ax, neg) = (fabs(x), x < 0.0);
    let (base, t) = if ax > 2.414_213_6 {
        (FRAC_PI_2, -1.0 / ax)
    } else if ax > 0.414_213_57 {
        (FRAC_PI_4, (ax - 1.0) / (ax + 1.0))
    } else {
        (0.0, ax)
    };
    let z = t * t;
    let y = base + (((8.053_744_5e-2 * z - 1.387_768_6e-1) * z + 1.997_771_1e-1) * z - 3.333_295e-1) * z * t + t;
    if neg { -y } else { y }
}

fn fatan2(y: f32, x: f32) -> f32 {
    if x == 0.0 {
        return if y > 0.0 {
            FRAC_PI_2
        } else if y < 0.0 {
            -FRAC_PI_2
        } else {
            0.0
        };
    }
    let a = fatan(y / x);
    if x > 0.0 {
        a
    } else if y >= 0.0 {
        a + PI
    } else {
        a - PI
    }
}

fn fsqrt(x: f32) -> f32 {
    if !(x > 0.0) {
        return if x == 0.0 { x } else { f32::NAN };
    }
    if x == f32::INFINITY {
        return x;
    }
    let mut y = f32::from_bits((x.to_bits() >> 1) + 0x1fbd_1df5);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    0.5 * (y + x / y)
}

fn fexp(x: f32) -> f32 {
    if x.is_nan() {
        return x;
    }
    if x > 88.722_84 {
        return f32::INFINITY;
    }
    if x < -103.972_08 {
        return 0.0;
    }
    let z = ffloor(1.442_695 * x + 0.5);
    let r = x - z * 0.693_359_4 - z * -2.121_944_4e-4;
    let p = (((((1.987_569_1e-4 * r + 1.398_2e-3) * r + 8.333_452e-3) * r + 4.166_579_6e-2) * r + 1.666_666_5e-1) * r + 5.000_000_1e-1)
        * r
        * r
        + r
        + 1.0;
    ldexp(p, z as i32)
}

fn fln(x: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x == 0.0 {
        return f32::NEG_INFINITY;
    }
    if x == f32::INFINITY {
        return x;
    }
    // x = m * 2^e with m in [0.5, 1)
    let (mut bits, mut e) = (x.to_bits(), 0i32);
    if bits < 0x0080_0000 {
        // subnormal: scale up first
        bits = (x * 16_777_216.0).to_bits();
        e -= 24;
    }
    e += ((bits >> 23) & 0xff) as i32 - 126;
    let mut m = f32::from_bits((bits & 0x007f_ffff) | 0x3f00_0000);
    if m < core::f32::consts::FRAC_1_SQRT_2 {
        e -= 1;
        m = m + m - 1.0;
    } else {
        m -= 1.0;
    }
    let z = m * m;
    let mut y = ((((((((7.037_683_6e-2 * m - 1.151_461e-1) * m + 1.167_699_9e-1) * m - 1.242_014_1e-1) * m + 1.424_932_3e-1)
        * m
        - 1.666_805_8e-1)
        * m
        + 2.000_071_4e-1)
        * m
        - 2.499_999_4e-1)
        * m
        + 3.333_333e-1)
        * m
        * z;
    let fe = e as f32;
    y += -2.121_944_4e-4 * fe;
    y += -0.5 * z;
    m + y + 0.693_359_4 * fe
}

/// Maths methods for `f32` that normally come from the standard library.
/// Pure Rust and deterministic. In the prelude.
pub trait Float: Sized {
    fn sqrt(self) -> f32;
    fn sin(self) -> f32;
    fn cos(self) -> f32;
    fn tan(self) -> f32;
    fn sin_cos(self) -> (f32, f32);
    fn asin(self) -> f32;
    fn acos(self) -> f32;
    fn atan(self) -> f32;
    fn atan2(self, x: f32) -> f32;
    fn exp(self) -> f32;
    fn ln(self) -> f32;
    fn log2(self) -> f32;
    fn log10(self) -> f32;
    fn powf(self, p: f32) -> f32;
    fn powi(self, n: i32) -> f32;
    fn floor(self) -> f32;
    fn ceil(self) -> f32;
    fn round(self) -> f32;
    fn trunc(self) -> f32;
    fn fract(self) -> f32;
    fn abs(self) -> f32;
    fn signum(self) -> f32;
    fn hypot(self, other: f32) -> f32;
    fn rem_euclid(self, rhs: f32) -> f32;
}

impl Float for f32 {
    fn sqrt(self) -> f32 {
        fsqrt(self)
    }
    fn sin(self) -> f32 {
        fsin(self)
    }
    fn cos(self) -> f32 {
        fcos(self)
    }
    fn tan(self) -> f32 {
        fsin(self) / fcos(self)
    }
    fn sin_cos(self) -> (f32, f32) {
        (fsin(self), fcos(self))
    }
    fn asin(self) -> f32 {
        fatan2(self, fsqrt((1.0 - self) * (1.0 + self)))
    }
    fn acos(self) -> f32 {
        fatan2(fsqrt((1.0 - self) * (1.0 + self)), self)
    }
    fn atan(self) -> f32 {
        fatan(self)
    }
    fn atan2(self, x: f32) -> f32 {
        fatan2(self, x)
    }
    fn exp(self) -> f32 {
        fexp(self)
    }
    fn ln(self) -> f32 {
        fln(self)
    }
    fn log2(self) -> f32 {
        fln(self) * core::f32::consts::LOG2_E
    }
    fn log10(self) -> f32 {
        fln(self) * core::f32::consts::LOG10_E
    }
    fn powf(self, p: f32) -> f32 {
        if p == 0.0 {
            return 1.0;
        }
        if self == 0.0 {
            return if p > 0.0 { 0.0 } else { f32::INFINITY };
        }
        if self < 0.0 {
            // only whole powers of negative numbers are real
            if ffloor(p) != p {
                return f32::NAN;
            }
            let v = fexp(p * fln(-self));
            let odd = fabs(p) < 16_777_216.0 && (p as i64) & 1 == 1;
            return if odd { -v } else { v };
        }
        fexp(p * fln(self))
    }
    fn powi(self, n: i32) -> f32 {
        let mut base = if n < 0 { 1.0 / self } else { self };
        let mut e = n.unsigned_abs();
        let mut r = 1.0;
        while e > 0 {
            if e & 1 == 1 {
                r *= base;
            }
            base *= base;
            e >>= 1;
        }
        r
    }
    fn floor(self) -> f32 {
        ffloor(self)
    }
    fn ceil(self) -> f32 {
        -ffloor(-self)
    }
    fn round(self) -> f32 {
        if self < 0.0 { -ffloor(-self + 0.5) } else { ffloor(self + 0.5) }
    }
    fn trunc(self) -> f32 {
        if self < 0.0 { -ffloor(-self) } else { ffloor(self) }
    }
    fn fract(self) -> f32 {
        self - self.trunc()
    }
    fn abs(self) -> f32 {
        fabs(self)
    }
    fn signum(self) -> f32 {
        if self.is_nan() {
            f32::NAN
        } else if self.is_sign_negative() {
            -1.0
        } else {
            1.0
        }
    }
    fn hypot(self, other: f32) -> f32 {
        fsqrt(self * self + other * other)
    }
    fn rem_euclid(self, rhs: f32) -> f32 {
        let r = self - rhs * ffloor(self / rhs);
        if r < 0.0 { r + fabs(rhs) } else { r }
    }
}

// ---------------------------------------------------------------- small helpers

/// Linear interpolation: `a` at t=0, `b` at t=1.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Move `from` towards `to` by at most `step`, without overshooting.
pub fn approach(from: f32, to: f32, step: f32) -> f32 {
    if from < to { (from + step).min(to) } else { (from - step).max(to) }
}

/// Wrap `v` into `0..max` (also for negative `v`), e.g. for looping worlds.
pub fn wrap(v: f32, max: f32) -> f32 {
    v.rem_euclid(max)
}

/// Wrap an integer into `0..max`.
pub fn wrapi(v: i32, max: i32) -> i32 {
    v.rem_euclid(max)
}

/// Integer square root.
pub fn isqrt(v: u32) -> u32 {
    if v < 2 {
        return v;
    }
    let mut x = fsqrt(v as f32) as u32;
    while x * x > v {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= v {
        x += 1;
    }
    x
}

// ---------------------------------------------------------------- Num

/// Numbers that can be used as screen coordinates or sizes: all integer types
/// and `f32`/`f64` (rounded down to the pixel).
pub trait Num: Copy {
    fn to_i32(self) -> i32;
    fn to_f32(self) -> f32;
}

macro_rules! num_int {
    ($($t:ty)*) => {$(
        impl Num for $t {
            #[inline] fn to_i32(self) -> i32 { self as i32 }
            #[inline] fn to_f32(self) -> f32 { self as f32 }
        }
    )*};
}
num_int!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize);

impl Num for f32 {
    #[inline]
    fn to_i32(self) -> i32 {
        ffloor(self) as i32
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self
    }
}

impl Num for f64 {
    #[inline]
    fn to_i32(self) -> i32 {
        ffloor(self as f32) as i32
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }
}

// ---------------------------------------------------------------- Vec2

/// A 2D vector (position, velocity, direction).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// Shorthand for `Vec2 { x, y }`; takes any numbers.
pub fn vec2(x: impl Num, y: impl Num) -> Vec2 {
    Vec2 { x: x.to_f32(), y: y.to_f32() }
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const ONE: Vec2 = Vec2 { x: 1.0, y: 1.0 };
    pub const UP: Vec2 = Vec2 { x: 0.0, y: -1.0 };
    pub const DOWN: Vec2 = Vec2 { x: 0.0, y: 1.0 };
    pub const LEFT: Vec2 = Vec2 { x: -1.0, y: 0.0 };
    pub const RIGHT: Vec2 = Vec2 { x: 1.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
    /// Unit vector pointing at `angle` radians (0 = right, PI/2 = down).
    pub fn from_angle(angle: f32) -> Vec2 {
        Vec2 { x: fcos(angle), y: fsin(angle) }
    }
    pub fn length(self) -> f32 {
        fsqrt(self.x * self.x + self.y * self.y)
    }
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
    /// Same direction, length 1 (or zero if this is zero).
    pub fn normalized(self) -> Vec2 {
        let l = self.length();
        if l > 0.0 { self / l } else { Vec2::ZERO }
    }
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    /// 2D cross product (z of the 3D one).
    pub fn cross(self, o: Vec2) -> f32 {
        self.x * o.y - self.y * o.x
    }
    pub fn distance(self, o: Vec2) -> f32 {
        (o - self).length()
    }
    /// Angle in radians (0 = right, PI/2 = down).
    pub fn angle(self) -> f32 {
        fatan2(self.y, self.x)
    }
    pub fn rotated(self, angle: f32) -> Vec2 {
        let (s, c) = (fsin(angle), fcos(angle));
        Vec2 { x: self.x * c - self.y * s, y: self.x * s + self.y * c }
    }
    /// Rotated a quarter turn (90 degrees).
    pub fn perp(self) -> Vec2 {
        Vec2 { x: -self.y, y: self.x }
    }
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }
    /// Shortened to at most `max` long.
    pub fn clamp_length(self, max: f32) -> Vec2 {
        let l = self.length();
        if l > max && l > 0.0 { self * (max / l) } else { self }
    }
    /// Moved towards `to` by at most `step`.
    pub fn approach(self, to: Vec2, step: f32) -> Vec2 {
        let d = to - self;
        let l = d.length();
        if l <= step || l == 0.0 { to } else { self + d * (step / l) }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2 { x: self.x + o.x, y: self.y + o.y }
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2 { x: self.x - o.x, y: self.y - o.y }
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2 { x: self.x * s, y: self.y * s }
    }
}
impl Mul<Vec2> for f32 {
    type Output = Vec2;
    fn mul(self, v: Vec2) -> Vec2 {
        v * self
    }
}
impl Mul for Vec2 {
    type Output = Vec2;
    fn mul(self, o: Vec2) -> Vec2 {
        Vec2 { x: self.x * o.x, y: self.y * o.y }
    }
}
impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f32) -> Vec2 {
        Vec2 { x: self.x / s, y: self.y / s }
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2 { x: -self.x, y: -self.y }
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
    }
}
impl DivAssign<f32> for Vec2 {
    fn div_assign(&mut self, s: f32) {
        self.x /= s;
        self.y /= s;
    }
}

// ---------------------------------------------------------------- Rect

/// An axis-aligned rectangle: top-left corner plus size (hit boxes, buttons).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Shorthand for `Rect { x, y, w, h }`; takes any numbers.
pub fn rect(x: impl Num, y: impl Num, w: impl Num, h: impl Num) -> Rect {
    Rect { x: x.to_f32(), y: y.to_f32(), w: w.to_f32(), h: h.to_f32() }
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    /// A `w` x `h` rectangle centred on `c`.
    pub fn centered(c: Vec2, w: f32, h: f32) -> Rect {
        Rect { x: c.x - w / 2.0, y: c.y - h / 2.0, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> Vec2 {
        Vec2 { x: self.x + self.w / 2.0, y: self.y + self.h / 2.0 }
    }
    /// Do the two rectangles overlap (touching edges don't count)?
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    /// Is the point inside?
    pub fn contains(&self, x: impl Num, y: impl Num) -> bool {
        let (x, y) = (x.to_f32(), y.to_f32());
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    /// Moved by (dx, dy).
    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect { x: self.x + dx, y: self.y + dy, ..*self }
    }
    /// Grown by `m` on every side (shrunk if negative).
    pub fn expand(&self, m: f32) -> Rect {
        Rect { x: self.x - m, y: self.y - m, w: self.w + 2.0 * m, h: self.h + 2.0 * m }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol * (1.0 + b.abs())
    }

    #[test]
    fn float_functions_match_std() {
        let mut x = -50.0f32;
        while x < 50.0 {
            assert!(close(Float::sin(x), x.sin(), 2e-6), "sin {x}");
            assert!(close(Float::cos(x), x.cos(), 2e-6), "cos {x}");
            assert!(close(Float::atan(x), x.atan(), 2e-6), "atan {x}");
            assert!(close(Float::floor(x), x.floor(), 0.0), "floor {x}");
            assert!(close(Float::ceil(x), x.ceil(), 0.0), "ceil {x}");
            assert!(close(Float::round(x), x.round(), 0.0), "round {x}");
            assert!(close(Float::exp(x * 0.5), (x * 0.5).exp(), 3e-6), "exp {x}");
            if x > 0.0 {
                assert!(close(Float::sqrt(x), x.sqrt(), 1e-6), "sqrt {x}");
                assert!(close(Float::ln(x), x.ln(), 2e-6), "ln {x}");
                assert!(close(Float::powf(x, 1.7), x.powf(1.7), 1e-5), "powf {x}");
            }
            for y in [-3.0f32, -0.5, 0.0, 0.25, 2.0] {
                assert!(close(Float::atan2(y, x), y.atan2(x), 2e-6), "atan2 {y} {x}");
            }
            x += 0.137;
        }
        assert_eq!(Float::powi(2.0f32, 10), 1024.0);
        assert_eq!(Float::sqrt(0.0f32), 0.0);
        assert!(Float::sqrt(-1.0f32).is_nan());
        assert!(close(Float::asin(0.5f32), 0.5f32.asin(), 2e-6));
        assert!(close(Float::acos(-0.3f32), (-0.3f32).acos(), 2e-6));
        assert!(close(Float::ln(1e-40f32), 1e-40f32.ln(), 2e-6));
    }

    #[test]
    fn num_rounds_down() {
        assert_eq!((-0.5f32).to_i32(), -1);
        assert_eq!(2.9f32.to_i32(), 2);
        assert_eq!(7u8.to_i32(), 7);
    }
}
