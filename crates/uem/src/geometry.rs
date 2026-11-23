//! Part geometry of the developed families (Spec 02 §4.3, §4.4): the parameter structs of the
//! oblique cone frustum and the cylinder-on-cylinder branch with their surface maps, and the
//! polyline chord-error limit. Neutral ground: the development engine (`develop`), the chord-bound
//! certification (`curve_bound`) and the verifier (`verify`) all consume these types, none owns
//! them, so the verifier's code needs no import from `develop` (work plan task 1.5, RC-15).
//! The rounding bounds of the engine's evaluated expressions stay in `develop` (they bound ITS
//! code and feed its reported `eps_num`); the verifier re-derives its own copy in `verify`, and a
//! test guards both the independence and the agreement of the two copies
//! (`tests/verifier_independence.rs`).

/// Largest distance between a polyline and the exact curve it stands for that this build accepts,
/// in millimetres. The number of segments of every polyline is doubled from its base value until
/// the measured distance is at or below this value. IMPLEMENTATION LIMIT pending engineering
/// review: the specification (ALG-07, §7.4) asks for a budget derived from the tolerance of the
/// part, which this build does not have. A part that cannot be represented within the limit with
/// `MAX_SEGMENTS` segments is refused, not drawn coarser.
pub const MAX_CHORD_ERROR_MM: f64 = 0.01;

/// Geometry of an oblique circular cone frustum between parallel planes.
#[derive(Clone, Copy, Debug)]
pub struct ObliqueCone {
    pub r1: f64,
    pub r2: f64,
    pub h: f64,
    pub e: f64,
}

impl ObliqueCone {
    /// Apex (a, 0, hh).
    pub fn apex(&self) -> (f64, f64) {
        let k = self.r1 / (self.r1 - self.r2);
        (self.e * k, self.h * k)
    }
    /// rho², written without cancellation: (r1 − a)² + H² + 4·a·r1·sin²(theta/2). Every term is
    /// non-negative. (The expanded form r1² − 2·a·r1·cos(theta) + a² + H² loses digits when the apex
    /// lies close to the rim; independent validation measured twice the stated error there, D4.)
    fn rho2(&self, th: f64) -> f64 {
        let (a, hh) = self.apex();
        let s = (0.5 * th).sin();
        (self.r1 - a) * (self.r1 - a) + hh * hh + 4.0 * a * self.r1 * s * s
    }
    pub fn rho(&self, th: f64) -> f64 {
        self.rho2(th).sqrt()
    }
    /// d(psi)/d(theta), Spec 02 eq. (4.1).
    pub fn dpsi(&self, th: f64) -> f64 {
        let (a, hh) = self.apex();
        let s = (0.5 * th).sin();
        let x = (self.r1 - a) + 2.0 * a * s * s; // r1 − a·cos(theta)
        self.r1 * hh.hypot(x) / self.rho2(th)
    }
}

/// Cylinder-on-cylinder branch, upper sheet. Spec 02 eq. (4.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Branch {
    /// Header contact radius.
    pub big_r: f64,
    /// Branch contact radius.
    pub r: f64,
    /// Angle between axes (rad), 0 < beta < pi.
    pub beta: f64,
    /// Offset of branch axis from header axis.
    pub delta: f64,
}

impl Branch {
    pub fn s(&self, phi: f64) -> f64 {
        let y = self.delta + self.r * phi.cos();
        ((self.big_r * self.big_r - y * y).sqrt() - self.r * phi.sin() * self.beta.cos())
            / self.beta.sin()
    }
    /// 3D point on the branch surface (header axis = x axis).
    pub fn point(&self, phi: f64, s: f64) -> [f64; 3] {
        let (sb, cb) = self.beta.sin_cos();
        [
            s * cb - self.r * phi.sin() * sb,
            self.delta + self.r * phi.cos(),
            s * sb + self.r * phi.sin() * cb,
        ]
    }
}
