//! Geometry of the cut profile, from the entities the reader gives: pieces (lines, arcs, polylines),
//! chains of open pieces into contours, exact areas and lengths, sampled outlines for the
//! intersection and inside tests.

use crate::reader::{Ent, P};

/// Relative digest tolerance: tol = TOL_REL x the part's bounding-box diagonal (both in mm).
/// About 0.001 mm for a 1 m part; it scales with the job, so scaled jobs get the same verdicts.
pub const TOL_REL: f64 = 1e-6;

/// Digest tolerance (mm) of a part whose bounding-box diagonal is `diag` (mm).
pub fn tolerance(diag: f64) -> f64 {
    if diag.is_finite() && diag > 0.0 {
        TOL_REL * diag
    } else {
        0.0
    }
}

/// Bounding-box diagonal (mm) of the finite points; 0 when there are none.
pub fn diagonal(pts: impl Iterator<Item = P>) -> f64 {
    bbox_of(pts.filter(|p| p[0].is_finite() && p[1].is_finite()))
        .map_or(0.0, |b| (b[2] - b[0]).hypot(b[3] - b[1]))
}

/// Sagitta used to sample arcs for the intersection and inside tests (mm).
const SAG: f64 = 1e-4;
/// Most chords per arc; an arc needing more is sampled coarsely and the intersection test is inconclusive.
const MAX_CHORDS: usize = 4096;
/// Most segments for the pairwise intersection test; more is inconclusive.
pub const MAX_SEGMENTS: usize = 20000;

#[derive(Clone, Copy, Debug)]
pub struct Arc {
    pub c: P,
    pub r: f64,
    /// Start angle (rad); the arc runs counter-clockwise for `sweep` (rad, 0 ≤ sweep < 2π).
    pub t0: f64,
    pub sweep: f64,
}

#[derive(Clone, Debug)]
pub struct Piece {
    pub layer: String,
    /// A closed polyline is a loop of its own; every other piece chains with its neighbours.
    pub closed: bool,
    /// Path of the piece in its own direction (for an arc: the chords of the sampled curve).
    pub pts: Vec<P>,
    pub arc: Option<Arc>,
    pub length: f64,
    /// Shortest segment (mm) of the piece.
    pub min_seg: f64,
    /// Segments shorter than the digest tolerance (an arc counts as one segment).
    pub tiny: usize,
    /// False where an arc needed more chords than MAX_CHORDS.
    pub sampled_ok: bool,
}

impl Piece {
    pub fn start(&self) -> P {
        match self.arc {
            Some(a) => arc_pt(&a, a.t0),
            None => self.pts[0],
        }
    }
    pub fn end(&self) -> P {
        match self.arc {
            Some(a) => arc_pt(&a, a.t0 + a.sweep),
            None => *self.pts.last().unwrap_or(&self.pts[0]),
        }
    }
    /// Middle point of the path, used to recognise a duplicate.
    pub fn mid(&self) -> P {
        self.pts[self.pts.len() / 2]
    }
}

fn arc_pt(a: &Arc, t: f64) -> P {
    [a.c[0] + a.r * t.cos(), a.c[1] + a.r * t.sin()]
}

fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// Build the pieces of the cut geometry. `empty` counts polylines without vertices (degenerate, dropped).
pub fn pieces(ents: &[Ent], tiny_min: f64) -> (Vec<Piece>, usize) {
    let mut out = Vec::new();
    let mut empty = 0;
    for e in ents {
        match e {
            Ent::Line { layer, a, b } => out.push(path_piece(layer, vec![*a, *b], false, tiny_min)),
            Ent::Poly { layer, pts, closed } => {
                if pts.is_empty() {
                    empty += 1;
                } else {
                    out.push(path_piece(layer, pts.clone(), *closed, tiny_min));
                }
            }
            Ent::Arc {
                layer,
                c,
                r,
                start_deg,
                end_deg,
            } => out.push(arc_piece(layer, *c, *r, *start_deg, *end_deg, tiny_min)),
            Ent::Text { .. } | Ent::Other { .. } => {}
        }
    }
    (out, empty)
}

fn path_piece(layer: &str, pts: Vec<P>, closed: bool, tiny_min: f64) -> Piece {
    let mut segs: Vec<f64> = pts.windows(2).map(|w| dist(w[0], w[1])).collect();
    if closed && pts.len() >= 2 {
        segs.push(dist(pts[pts.len() - 1], pts[0]));
    }
    let length: f64 = segs.iter().sum();
    let min_seg = segs.iter().copied().fold(f64::INFINITY, f64::min);
    let tiny = segs.iter().filter(|s| **s < tiny_min).count() + usize::from(pts.len() < 2);
    Piece {
        layer: layer.to_string(),
        closed,
        pts,
        arc: None,
        length,
        min_seg: if min_seg.is_finite() { min_seg } else { 0.0 },
        tiny,
        sampled_ok: true,
    }
}

fn arc_piece(layer: &str, c: P, r: f64, start_deg: f64, end_deg: f64, tiny_min: f64) -> Piece {
    let tau = std::f64::consts::TAU;
    let t0 = start_deg.to_radians();
    let sweep = (end_deg.to_radians() - t0).rem_euclid(tau);
    let arc = Arc { c, r, t0, sweep };
    let length = r.abs() * sweep;
    let step = if r.abs() > SAG {
        2.0 * (1.0 - SAG / r.abs()).acos()
    } else {
        sweep.max(1e-9)
    };
    let wanted = if step > 0.0 {
        (sweep / step).ceil() as usize
    } else {
        MAX_CHORDS + 1
    };
    let n = wanted.clamp(1, MAX_CHORDS);
    let sampled_ok = wanted <= MAX_CHORDS;
    let pts: Vec<P> = (0..=n)
        .map(|k| arc_pt(&arc, t0 + sweep * k as f64 / n as f64))
        .collect();
    let min_seg = if sweep > 0.0 { length } else { 0.0 };
    Piece {
        layer: layer.to_string(),
        closed: false,
        pts,
        arc: Some(arc),
        length,
        min_seg,
        tiny: usize::from(length < tiny_min),
        sampled_ok,
    }
}

impl Piece {
    /// Exact bounding box [minx, miny, maxx, maxy] of the piece.
    pub fn bbox(&self) -> [f64; 4] {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        let mut add = |p: P| {
            b[0] = b[0].min(p[0]);
            b[1] = b[1].min(p[1]);
            b[2] = b[2].max(p[0]);
            b[3] = b[3].max(p[1]);
        };
        match self.arc {
            Some(a) => {
                add(self.start());
                add(self.end());
                for k in 0..4 {
                    let th = k as f64 * std::f64::consts::FRAC_PI_2;
                    if (th - a.t0).rem_euclid(std::f64::consts::TAU) <= a.sweep {
                        add(arc_pt(&a, th));
                    }
                }
            }
            None => self.pts.iter().for_each(|p| add(*p)),
        }
        b
    }

    /// Signed twice-area contribution of the piece traversed forward (`fwd`) or backward.
    fn contribution(&self, fwd: bool) -> f64 {
        let s = match self.arc {
            Some(a) => {
                let t1 = a.t0 + a.sweep;
                a.c[0] * a.r * (t1.sin() - a.t0.sin())
                    + a.c[1] * a.r * (a.t0.cos() - t1.cos())
                    + a.r * a.r * a.sweep
            }
            None => {
                let n = self.pts.len();
                let mut s: f64 = self.pts.windows(2).map(|w| cross(w[0], w[1])).sum();
                if self.closed && n >= 2 {
                    s += cross(self.pts[n - 1], self.pts[0]);
                }
                s
            }
        };
        if fwd { s } else { -s }
    }

    /// Path points in the traversal direction.
    fn oriented(&self, fwd: bool) -> Vec<P> {
        if fwd {
            self.pts.clone()
        } else {
            self.pts.iter().rev().copied().collect()
        }
    }
}

/// A contour: a closed loop of pieces, or an open chain with its gap.
#[derive(Clone, Debug)]
pub struct Contour {
    pub layer: String,
    pub closed: bool,
    /// Area enclosed (mm²), closed contours only.
    pub area: f64,
    pub length: f64,
    /// Points around a closed contour, in order (sampled arcs), for the inside test.
    pub samples: Vec<P>,
    /// Distance between the two free ends of an open chain (mm); 0 when closed.
    pub gap: f64,
    /// Joints where three or more ends meet (a branching chain).
    pub branch_joints: usize,
}

/// Chain the open pieces into contours and collect the closed polylines as contours of their own.
pub fn contours(pieces: &[Piece], tol: f64) -> Vec<Contour> {
    let mut out = Vec::new();
    for p in pieces.iter().filter(|p| p.closed) {
        out.push(Contour {
            layer: p.layer.clone(),
            closed: true,
            area: (p.contribution(true) / 2.0).abs(),
            length: p.length,
            samples: p.pts.clone(),
            gap: 0.0,
            branch_joints: 0,
        });
    }
    let open: Vec<&Piece> = pieces.iter().filter(|p| !p.closed).collect();
    let m = 2 * open.len();
    let pos: Vec<P> = open.iter().flat_map(|p| [p.start(), p.end()]).collect();
    // union-find over the ends: ends within tol of each other are one joint
    let mut parent: Vec<usize> = (0..m).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for i in 0..m {
        for j in (i + 1)..m {
            if dist(pos[i], pos[j]) <= tol {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                if a != b {
                    parent[a] = b;
                }
            }
        }
    }
    let roots: Vec<usize> = (0..m).map(|i| find(&mut parent, i)).collect();
    let mut joint: Vec<Vec<usize>> = vec![Vec::new(); m];
    for (i, r) in roots.iter().enumerate() {
        joint[*r].push(i);
    }
    // components of pieces: pieces sharing a joint
    let mut comp: Vec<usize> = (0..open.len()).collect();
    fn cfind(comp: &mut [usize], mut x: usize) -> usize {
        while comp[x] != x {
            comp[x] = comp[comp[x]];
            x = comp[x];
        }
        x
    }
    for (k, _) in open.iter().enumerate() {
        for end in [2 * k, 2 * k + 1] {
            let other = joint[roots[end]].iter().copied().find(|&e| e != end);
            if let Some(e) = other {
                let (a, b) = (cfind(&mut comp, k), cfind(&mut comp, e / 2));
                if a != b {
                    comp[a] = b;
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); open.len()];
    for k in 0..open.len() {
        let r = cfind(&mut comp, k);
        groups[r].push(k);
    }
    for group in groups.into_iter().filter(|g| !g.is_empty()) {
        let joints: Vec<&Vec<usize>> = group
            .iter()
            .flat_map(|&k| [2 * k, 2 * k + 1])
            .map(|e| &joint[roots[e]])
            .collect();
        let closed_chain = joints.iter().all(|j| j.len() == 2);
        let length: f64 = group.iter().map(|&k| open[k].length).sum();
        let layer = open[group[0]].layer.clone();
        if closed_chain && let Some((area2, samples)) = walk(&open, &group, &joint, &roots) {
            out.push(Contour {
                layer,
                closed: true,
                area: (area2 / 2.0).abs(),
                length,
                samples,
                gap: 0.0,
                branch_joints: 0,
            });
            continue;
        }
        // open or branching: the free ends give the gap
        let free: Vec<P> = group
            .iter()
            .flat_map(|&k| [2 * k, 2 * k + 1])
            .filter(|&e| joint[roots[e]].len() == 1)
            .map(|e| pos[e])
            .collect();
        let mut gap = 0.0f64;
        for i in 0..free.len() {
            for j in (i + 1)..free.len() {
                gap = gap.max(dist(free[i], free[j]));
            }
        }
        let branch_joints = joints.iter().filter(|j| j.len() > 2).count();
        out.push(Contour {
            layer,
            closed: false,
            area: 0.0,
            length,
            samples: Vec::new(),
            gap,
            branch_joints,
        });
    }
    out
}

/// Walk a closed chain of pieces: its signed twice-area and its points in order. None if the walk does not
/// return to its start within the number of pieces (a defect the caller reports as an open contour).
fn walk(
    open: &[&Piece],
    group: &[usize],
    joint: &[Vec<usize>],
    roots: &[usize],
) -> Option<(f64, Vec<P>)> {
    let start = group[0];
    let (mut k, mut enter) = (start, 0usize);
    let mut area2 = 0.0;
    let mut samples = Vec::new();
    for _ in 0..=group.len() {
        let fwd = enter == 0;
        let p = open[k];
        area2 += p.contribution(fwd);
        for (i, q) in p.oriented(fwd).into_iter().enumerate() {
            if i == 0 && !samples.is_empty() {
                continue;
            }
            samples.push(q);
        }
        let exit = 2 * k + (1 - enter);
        let j = &joint[roots[exit]];
        if j.len() != 2 {
            return None;
        }
        let next = if j[0] == exit { j[1] } else { j[0] };
        let (nk, ne) = (next / 2, next % 2);
        if nk == start {
            return if ne == 0 {
                Some((area2, samples))
            } else {
                None
            };
        }
        k = nk;
        enter = ne;
    }
    None
}

/// Pairs of segments that cross properly (not at a shared end). Returns (count, inconclusive).
pub fn crossings(pieces: &[&Piece], tol: f64) -> (usize, bool) {
    let mut segs: Vec<(P, P)> = Vec::new();
    let mut sampled = true;
    for p in pieces {
        sampled &= p.sampled_ok;
        let n = p.pts.len();
        for w in p.pts.windows(2) {
            segs.push((w[0], w[1]));
        }
        if p.closed && n >= 2 {
            segs.push((p.pts[n - 1], p.pts[0]));
        }
    }
    if !sampled || segs.len() > MAX_SEGMENTS {
        return (0, true);
    }
    let bb: Vec<[f64; 4]> = segs
        .iter()
        .map(|(a, b)| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[0].max(b[0]),
                a[1].max(b[1]),
            ]
        })
        .collect();
    let mut count = 0;
    for i in 0..segs.len() {
        for j in (i + 1)..segs.len() {
            let (p, q) = (bb[i], bb[j]);
            if p[2] < q[0] - tol || q[2] < p[0] - tol || p[3] < q[1] - tol || q[3] < p[1] - tol {
                continue;
            }
            let ((a, b), (c, d)) = (segs[i], segs[j]);
            let touching = [a, b]
                .iter()
                .any(|x| [c, d].iter().any(|y| dist(*x, *y) <= tol));
            if !touching && proper(a, b, c, d) {
                count += 1;
            }
        }
    }
    (count, false)
}

fn orient(a: P, b: P, c: P) -> f64 {
    cross([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]])
}

fn proper(a: P, b: P, c: P, d: P) -> bool {
    let (d1, d2) = (orient(c, d, a), orient(c, d, b));
    let (d3, d4) = (orient(a, b, c), orient(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Pairs of segments that repeat each other: the same two ends (either direction) and the same middle.
/// A polyline contributes each of its segments; an arc contributes once.
pub fn duplicates(pieces: &[&Piece], tol: f64) -> usize {
    let mut units: Vec<(P, P, P)> = Vec::new();
    for p in pieces {
        match p.arc {
            Some(_) => units.push((p.start(), p.end(), p.mid())),
            None => {
                let n = p.pts.len();
                for w in p.pts.windows(2) {
                    units.push((
                        w[0],
                        w[1],
                        [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0],
                    ));
                }
                if p.closed && n >= 2 {
                    let (a, b) = (p.pts[n - 1], p.pts[0]);
                    units.push((a, b, [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]));
                }
            }
        }
    }
    let mut n = 0;
    for i in 0..units.len() {
        for j in (i + 1)..units.len() {
            let ((a, b, m), (c, d, k)) = (units[i], units[j]);
            let same_ends = (dist(a, c) <= tol && dist(b, d) <= tol)
                || (dist(a, d) <= tol && dist(b, c) <= tol);
            if same_ends && dist(m, k) <= tol {
                n += 1;
            }
        }
    }
    n
}

/// Even-odd test of a point against a closed sample polygon.
pub fn inside(poly: &[P], p: P) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Area of a closed loop of points (shoelace), absolute value.
pub fn loop_area(pts: &[P]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let s: f64 = (0..n).map(|i| cross(pts[i], pts[(i + 1) % n])).sum();
    (s / 2.0).abs()
}

/// Bounding box of a set of points, or None when empty.
pub fn bbox_of(pts: impl Iterator<Item = P>) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for p in pts {
        b = Some(match b {
            None => [p[0], p[1], p[0], p[1]],
            Some(x) => [
                x[0].min(p[0]),
                x[1].min(p[1]),
                x[2].max(p[0]),
                x[3].max(p[1]),
            ],
        });
    }
    b
}

/// Radius of the circle through three points, None when they are (nearly) collinear.
pub fn circumradius(a: P, b: P, c: P) -> Option<f64> {
    let d = 2.0 * cross([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
    if d.abs() < 1e-12 {
        return None;
    }
    let ab = dist(a, b);
    let bc = dist(b, c);
    let ca = dist(c, a);
    Some(ab * bc * ca / d.abs())
}
