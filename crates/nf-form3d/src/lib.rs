//! Formed 3D geometry of the developable families (A-20).
//!
//! `Form3d::map` takes a point of a flat pattern, in the plate coordinates of the engine's pattern (mm), and returns
//! the position of the same point on the formed part (mm). The map is an isometry: a length on the plate is the
//! length of its image on the surface. Only the families with a closed-form map are here: the cylinder shell and
//! the right circular cone frustum (concentric reducer), as `uem::develop` produces them. Pure library: no I/O.
//!
//! Frames and units: lengths in mm, angles in radians.
//!
//! Cylinder (`Form3d::cylinder(radius, length)`, pattern `cylinder_shell`)
//! - Plate: u is the arc length along the circumference from the seam `seam_a` at u = 0, u in [0, 2*pi*radius];
//!   v is the distance along the axis from the end `bottom` at v = 0, v in [0, length].
//! - Formed: the axis is the 3D z axis through x = y = 0. The end v = 0 is at z = 0, the end v = length at z = length.
//!   Position = (radius*cos(u/radius), radius*sin(u/radius), v).
//!
//! Right cone (`Form3d::cone(r1, r2, h)`, pattern `cone_frustum`; r1 > r2 > 0 the end radii, h the axial height)
//! - Plate: the origin (0, 0) is the apex of the developed cone, 0 mm inside the small-end arc. The developed
//!   frustum is an annular sector between the radius l2 (small end) and l1 (large end), with l1 = r1/sin(alpha),
//!   l2 = r2/sin(alpha), where sin(alpha) = (r1 - r2)/slant and slant = hypot(h, r1 - r2). The seam `seam_a` lies on
//!   the +x axis, and the sector angle is phi = 2*pi*sin(alpha). A plate point is (x, y) with distance l from the
//!   origin and polar angle psi in [0, phi].
//! - Formed: the axis is the 3D z axis, pointing from the small end toward the large end. The small end is the
//!   circle z = 0 of radius r2, the large end the circle z = h of radius r1. The apex lies at
//!   z = -r2*h/(r1 - r2). The plate point maps to (l*sin(alpha)*cos(theta), -l*sin(alpha)*sin(theta),
//!   (l - l2)*cos(alpha)) with theta = psi/sin(alpha) in [0, 2*pi]. The surface is the set of points with
//!   rho = r2 + (r1 - r2)*z/h, rho = hypot(x, y).
//!
//! Orientation: for both families the plate axes (x, y) in the order given above are right-handed with the outward
//! surface normal, so the plate seen from +z is the view from outside (`uem::MARKING_SIDE`). The angle of the cone
//! increases counter-clockwise seen from the small end looking toward the large end, and the angle of the cylinder
//! counter-clockwise seen from the end v = length looking toward v = 0. These are the conventions the engine states.

use uem::{P2, Pattern, TAU};

/// A formed part of one of the supported families, with the derived values the map needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Form3d {
    /// Cylinder shell: circumference radius and length, both in mm.
    Cylinder { radius: f64, length: f64 },
    /// Right circular cone frustum. `sin_a` and `cos_a` are the sine and cosine of the half angle alpha, `slant` is
    /// the slant length between the end circles (mm), and `r1`, `r2`, `h` are the end radii and the axial height.
    Cone {
        r1: f64,
        r2: f64,
        h: f64,
        sin_a: f64,
        cos_a: f64,
        slant: f64,
    },
}

/// Refusal of a family or dimension this crate cannot map.
#[derive(Clone, Debug, PartialEq)]
pub enum Form3dError {
    /// A dimension is not finite and positive. The name is the parameter.
    NotPositive(&'static str),
    /// Cone radii must satisfy r1 > r2 (large end first).
    RadiiOrder,
    /// The pattern belongs to a family without a closed-form 3D map here.
    UnsupportedFamily(&'static str),
    /// A quantity the family needs is missing from the pattern.
    MissingQuantity(&'static str),
}

impl std::fmt::Display for Form3dError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Form3dError::NotPositive(name) => write!(f, "{name} must be finite and positive"),
            Form3dError::RadiiOrder => {
                write!(f, "cone radii must satisfy r1 > r2 (large end first)")
            }
            Form3dError::UnsupportedFamily(name) => {
                write!(f, "family {name} has no closed-form 3D map in nf-form3d")
            }
            Form3dError::MissingQuantity(name) => write!(f, "pattern quantity {name} is missing"),
        }
    }
}

impl std::error::Error for Form3dError {}

fn positive(name: &'static str, v: f64) -> Result<(), Form3dError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(Form3dError::NotPositive(name))
    }
}

/// The representative of the direction `p` (radians, in (-pi, pi]) that lies nearest to the sector [0, phi].
/// Directions of the sector seen by atan2 as slightly negative (a seam at angle 0) or as phi - 2*pi (a seam at a
/// sector angle above pi) are thereby mapped to the correct end of the sector.
fn nearest_in_sector(p: f64, phi: f64) -> f64 {
    let dist = |x: f64| (-x).max(x - phi).max(0.0);
    [p - TAU, p, p + TAU]
        .into_iter()
        .fold(p, |best, c| if dist(c) < dist(best) { c } else { best })
}

impl Form3d {
    /// A cylinder shell of the given radius and length (mm).
    pub fn cylinder(radius: f64, length: f64) -> Result<Form3d, Form3dError> {
        positive("radius", radius)?;
        positive("length", length)?;
        Ok(Form3d::Cylinder { radius, length })
    }

    /// A right circular cone frustum with end radii r1 > r2 and axial height h (mm).
    pub fn cone(r1: f64, r2: f64, h: f64) -> Result<Form3d, Form3dError> {
        positive("r1", r1)?;
        positive("r2", r2)?;
        positive("h", h)?;
        if r1 <= r2 {
            return Err(Form3dError::RadiiOrder);
        }
        let slant = h.hypot(r1 - r2);
        Ok(Form3d::Cone {
            r1,
            r2,
            h,
            sin_a: (r1 - r2) / slant,
            cos_a: h / slant,
            slant,
        })
    }

    /// The formed part of an engine pattern, taken from the pattern's own quantities.
    pub fn from_pattern(p: &Pattern) -> Result<Form3d, Form3dError> {
        let q = |name: &'static str| p.quantity(name).ok_or(Form3dError::MissingQuantity(name));
        match p.family {
            "cylinder_shell" => Form3d::cylinder(q("developed_width")? / TAU, q("length")?),
            "cone_frustum" => {
                let (l1, l2) = (q("apex_radius_large")?, q("apex_radius_small")?);
                let sin_a = q("sector_angle")? / TAU;
                let cos_a = (1.0 - sin_a * sin_a).sqrt();
                let slant = q("slant_length")?;
                Form3d::cone(l1 * sin_a, l2 * sin_a, slant * cos_a)
            }
            other => Err(Form3dError::UnsupportedFamily(other)),
        }
    }

    /// Largest dimension of the formed part (mm): the larger of the diameter and the axial length. It scales the
    /// tolerance of the checks, which are 1e-9 times this value.
    pub fn largest_dimension(&self) -> f64 {
        match *self {
            Form3d::Cylinder { radius, length } => (2.0 * radius).max(length),
            Form3d::Cone { r1, h, .. } => (2.0 * r1).max(h),
        }
    }

    /// Position (x, y, z) in mm of the plate point `uv` (mm), in the frame described in the module documentation.
    /// Points outside the developed domain are mapped by the same formula; the cone takes the nearest equivalent
    /// of their polar angle in the sector.
    pub fn map(&self, uv: P2) -> [f64; 3] {
        match *self {
            Form3d::Cylinder { radius, .. } => {
                let t = uv.x / radius;
                [radius * t.cos(), radius * t.sin(), uv.y]
            }
            Form3d::Cone {
                r2, sin_a, cos_a, ..
            } => {
                let l = uv.x.hypot(uv.y);
                let psi = nearest_in_sector(uv.y.atan2(uv.x), TAU * sin_a);
                let theta = psi / sin_a;
                let rho = l * sin_a;
                let l2 = r2 / sin_a;
                [rho * theta.cos(), -rho * theta.sin(), (l - l2) * cos_a]
            }
        }
    }
}
