"""Reference patterns of the five product families, built in 3D and developed
with reference.ruled.  Conventions chosen here (not the product's):

  * single parts live in a world frame with the part axis along +Z;
  * assemblies live in a deliberately oblique world frame (header axis
    (2,-1,2)/3) so that no coordinate coincidence can hide a frame error;
  * every reference pattern is seen from the OUTSIDE of the part;
  * the pattern's position in the plane is whatever the developer produces -
    the checks map frames with an explicit rigid motion, never by assumption.

Input semantics are taken from Specification 02 only:
  cone/eccentric (4.2, 4.3): C1 radius r1 at z=0, C2 radius r2 at z=h, centre
      offset e along +X, rulings join equal azimuth; seam ruling at the azimuth
      of the offset direction (theta = 0).
  branch (4.4): header axis a, branch axis direction b at angle beta to a,
      branch axis offset delta along o = (b x a)/|b x a|, branch azimuth phi
      measured from o, counter-clockwise about b; s measured along b from the
      foot of the common perpendicular; "upper sheet" = the intersection on the
      side from which the branch arrives.
  header plate: station measured along a from one plate end; longitudinal seam
      diametrically opposite the branch side (the product's stated assumption).
"""
import numpy as np
from .ruled import Circle3D, RuledSurface, Development

TAU = 2 * np.pi
EX, EY, EZ = np.eye(3)


class RefCurve:
    def __init__(self, name, f, t0, t1):
        self.name, self.f, self.t0, self.t1 = name, f, t0, t1

    def sample(self, n):
        return self.f(np.linspace(self.t0, self.t1, n))


class RefPattern:
    def __init__(self, family):
        self.family = family
        self.curves = {}
        self.info = {}

    def add(self, name, f, t0, t1):
        self.curves[name] = RefCurve(name, f, t0, t1)


class CylChart:
    """Development of a right circular cylinder obtained from the generic
    developer; the developer shows it to be an affine chart, which is then
    used for fast evaluation and for the inverse (fold-back) map."""

    def __init__(self, centre, radius, e1, e2, axis, scale):
        self.centre, self.radius, self.e1, self.e2, self.axis = map(np.asarray, (centre, radius, e1, e2, axis))
        self.radius = float(radius)
        circ = Circle3D(centre, radius, e1, e2)
        surf = RuledSurface(circ, lambda t: np.broadcast_to(self.axis, (len(t), 3)))
        dev = Development(surf, 0.0, TAU, scale=scale)
        self.c0 = dev.place(0.0, 0.0)[0]
        end = dev.place(TAU, 0.0)[0]
        self.eu = (end - self.c0) / np.linalg.norm(end - self.c0)
        self.ev = dev.place(0.0, 1.0)[0] - self.c0
        # the developer must have produced a straight directrix of length 2 pi r and unit, perpendicular rulings
        ts = np.linspace(0, TAU, 33)
        line = self.c0 + (self.radius * ts)[:, None] * self.eu
        self.straightness = float(np.max(np.linalg.norm(dev.place(ts, 0.0) - line, axis=1)))
        far = dev.place(ts, 1.0) - (line + self.ev)
        self.straightness = max(self.straightness, float(np.max(np.linalg.norm(far, axis=1))))
        assert self.straightness < 1e-10 * max(scale, 1.0), self.straightness
        assert abs(np.dot(self.eu, self.ev)) < 1e-10 and abs(np.linalg.norm(self.ev) - 1) < 1e-10

    def place(self, ang, lam):
        ang = np.atleast_1d(np.asarray(ang, float))
        lam = np.broadcast_to(np.asarray(lam, float), ang.shape)
        return self.c0 + (self.radius * ang)[:, None] * self.eu + lam[:, None] * self.ev

    def inverse(self, pts):
        d = np.asarray(pts, float) - self.c0
        return d @ self.eu / self.radius, d @ self.ev

    def point3d(self, ang, lam, radius=None):
        r = self.radius if radius is None else radius
        ang = np.atleast_1d(np.asarray(ang, float))[:, None]
        lam = np.atleast_1d(np.asarray(lam, float))[:, None]
        return self.centre + r * (np.cos(ang) * self.e1 + np.sin(ang) * self.e2) + lam * self.axis


# ---------------------------------------------------------------- cylinder
def cylinder_shell(r_dev, length):
    scale = max(TAU * r_dev, length)
    ch = CylChart(np.zeros(3), r_dev, EX, EY, EZ, scale)
    p = RefPattern('cylinder_shell')
    p.chart = ch
    p.add('end0', lambda t: ch.place(t, 0.0), 0.0, TAU)
    p.add('end1', lambda t: ch.place(t, length), 0.0, TAU)
    p.add('seam0', lambda lam: ch.place(np.zeros_like(np.atleast_1d(lam)), lam), 0.0, length)
    p.add('seam1', lambda lam: ch.place(np.full_like(np.atleast_1d(np.asarray(lam, float)), TAU), lam), 0.0, length)
    p.info.update(scale=scale, width=float(np.linalg.norm(ch.place(TAU, 0)[0] - ch.c0)), length=length)
    return p


# ---------------------------------------------------------------- cones
class ConeRef(RefPattern):
    pass


def _general_cone(family, r1, r2, h, e):
    c1 = Circle3D(np.zeros(3), r1, EX, EY)
    c2 = Circle3D(np.array([e, 0.0, h]), r2, EX, EY)
    surf = RuledSurface(c1, lambda t: c2.p(t) - c1.p(t))
    # apex: point of concurrency of the rulings, found numerically from two rulings (least squares)
    ta, tb = 0.3, 2.9
    A_ = np.stack([surf.D(ta)[0], -surf.D(tb)[0]], axis=1)
    lam = np.linalg.lstsq(A_, (c1.p(tb) - c1.p(ta))[0], rcond=None)[0]
    apex3 = c1.p(ta)[0] + lam[0] * surf.D(ta)[0]
    lam_apex = float(lam[0])
    scale = float(np.max(np.linalg.norm(c1.p(np.linspace(0, TAU, 9)) - apex3, axis=1)))
    dev = Development(surf, 0.0, TAU, scale=scale)
    p = ConeRef(family)
    p.surf, p.dev, p.c1, p.c2, p.apex3, p.lam_apex = surf, dev, c1, c2, apex3, lam_apex
    p.add('large', lambda t: dev.place(t, 0.0), 0.0, TAU)
    p.add('small', lambda t: dev.place(t, 1.0), 0.0, TAU)
    p.add('seam0', lambda lam: dev.place(np.zeros_like(np.atleast_1d(np.asarray(lam, float))), lam), 0.0, 1.0)
    p.add('seam1', lambda lam: dev.place(np.full_like(np.atleast_1d(np.asarray(lam, float)), TAU), lam), 0.0, 1.0)
    p.apex2 = dev.place(0.0, lam_apex)[0]
    # consistency of the developer: every ruling must pass through the developed apex
    ts = np.linspace(0, TAU, 41)
    p.info.update(scale=scale, apex_spread=float(np.max(np.linalg.norm(dev.place(ts, lam_apex) - p.apex2, axis=1))),
                  developability_defect=float(np.max(surf.developability_defect(ts))))
    # tables for the fold-back (pattern polar angle about the apex -> azimuth)
    tt = np.linspace(0, TAU, 20001)
    v = dev.place(tt, 0.0) - p.apex2
    ang = np.unwrap(np.arctan2(v[:, 1], v[:, 0]))
    p._ang0, p._sign = ang[0], np.sign(ang[-1] - ang[0])
    p._psi_tab, p._t_tab = (ang - ang[0]) * p._sign, tt
    p.info['psi_total'] = float(p._psi_tab[-1])
    return p


def _cone_fold(p, pts_ref):
    """Reference-frame pattern points -> (azimuth, 3D point)."""
    from scipy.interpolate import CubicSpline
    if not hasattr(p, '_spl'):
        p._spl = CubicSpline(p._psi_tab, p._t_tab)
    v = np.asarray(pts_ref, float) - p.apex2
    rho = np.linalg.norm(v, axis=1)
    ang = np.arctan2(v[:, 1], v[:, 0])
    psi = (ang - p._ang0) * p._sign
    psi = np.mod(psi + 0.5 * (TAU - p.info['psi_total']), TAU) - 0.5 * (TAU - p.info['psi_total'])  # centre the gap
    th = p._spl(psi)
    w = p.c1.p(th) - p.apex3
    rho3 = np.linalg.norm(w, axis=1)
    return th, p.apex3 + (rho / rho3)[:, None] * w, rho, rho3


ConeRef.fold = _cone_fold


def cone_frustum(r1, r2, h):
    return _general_cone('cone_frustum', r1, r2, h, 0.0)


def eccentric_reducer(r1, r2, h, e):
    return _general_cone('eccentric_reducer', r1, r2, h, e)


# ---------------------------------------------------------------- branch / header
class Joint3D:
    """Header cylinder and branch cylinder placed in an oblique world frame."""
    A = np.array([2.0, -1.0, 2.0]) / 3.0           # header axis
    T = np.array([1.0, 2.0, 0.0]) / np.sqrt(5.0)   # transverse direction toward which the branch leaves
    ORIGIN = np.array([37.0, -11.0, 5.0])          # header axis point at station 0

    def __init__(self, R, r, beta_deg, delta, station=0.0):
        self.R, self.r, self.delta, self.station = float(R), float(r), float(delta), float(station)
        self.beta = np.radians(beta_deg)
        a, t = self.A, self.T
        self.b = np.cos(self.beta) * a + np.sin(self.beta) * t   # branch axis, pointing away from the header
        bxa = np.cross(self.b, a)
        self.o = bxa / np.linalg.norm(bxa)                       # offset direction (Spec 02 4.4: +y)
        self.e2 = np.cross(self.b, self.o)                       # phi counter-clockwise about b, from o
        self.Q = self.ORIGIN + self.station * a + self.delta * self.o  # foot of the common perpendicular on the branch axis

    def branch_point(self, phi, s, radius=None):
        r = self.r if radius is None else radius
        phi = np.atleast_1d(np.asarray(phi, float))[:, None]
        s = np.atleast_1d(np.asarray(s, float))[:, None]
        return self.Q + s * self.b + r * (np.cos(phi) * self.o + np.sin(phi) * self.e2)

    def dist_header_axis(self, X):
        d = X - self.ORIGIN
        return np.linalg.norm(d - (d @ self.A)[:, None] * self.A, axis=1)

    def dist_branch_axis(self, X):
        d = X - self.Q
        return np.linalg.norm(d - (d @ self.b)[:, None] * self.b, axis=1)

    def s_of_phi(self, phi):
        """Intersection of each branch ruling with the header cylinder, by bracketing
        and bisection of the implicit function  f(s) = dist(P(phi,s), header axis)^2 - R^2
        on the side from which the branch arrives (largest root).  NaN where the
        ruling misses the header."""
        phi = np.atleast_1d(np.asarray(phi, float))
        P0 = self.branch_point(phi, np.zeros_like(phi)) - self.ORIGIN
        a = self.A
        w0 = P0 - (P0 @ a)[:, None] * a
        w1 = self.b - (self.b @ a) * a
        n1 = w1 @ w1

        def f(s):
            w = w0 + s[:, None] * w1
            return (w * w).sum(1) - self.R ** 2

        lo = -(w0 @ w1) / n1
        hi = lo + 2.0 * (self.R + np.linalg.norm(w0, axis=1) + 1.0) / np.sqrt(n1)
        miss = f(lo) >= 0
        for _ in range(64):
            mid = 0.5 * (lo + hi)
            neg = f(mid) < 0
            lo = np.where(neg, mid, lo)
            hi = np.where(neg, hi, mid)
        s = 0.5 * (lo + hi)
        s[miss] = np.nan
        return s

    def curve3d(self, phi):
        return self.branch_point(phi, self.s_of_phi(phi))

    def header_coords(self, X):
        """(station, angle from the seam) of 3D points; seam diametrically opposite T."""
        d = X - self.ORIGIN
        x = d @ self.A
        ang = np.arctan2(d @ self.o, d @ self.T)      # from T toward o
        return x, ang + np.pi

    def header_point(self, x, gamma, radius=None):
        R = self.R if radius is None else radius
        x = np.atleast_1d(np.asarray(x, float))[:, None]
        g = np.atleast_1d(np.asarray(gamma, float))[:, None] - np.pi
        return self.ORIGIN + x * self.A + R * (np.cos(g) * self.T + np.sin(g) * self.o)


def branch_stub(R, r, r_dev, beta_deg, delta, s_end, station=0.0):
    j = Joint3D(R, r, beta_deg, delta, station)
    scale = max(TAU * r_dev, s_end, R)
    # reference surface of the branch: radius r_dev, directrix = square-end circle, rulings toward the header
    ch = CylChart(j.Q + s_end * j.b, r_dev, j.o, j.e2, -j.b, scale)
    p = RefPattern('branch_stub')
    p.joint, p.chart, p.s_end = j, ch, s_end
    cut = lambda phi: ch.place(phi, s_end - j.s_of_phi(phi))
    s0 = float(j.s_of_phi(0.0)[0])
    p.add('square_end', lambda phi: ch.place(phi, 0.0), 0.0, TAU)
    p.add('saddle', cut, 0.0, TAU)
    p.add('seam0', lambda lam: ch.place(np.zeros_like(np.atleast_1d(np.asarray(lam, float))), lam), 0.0, s_end - s0)
    p.add('seam1', lambda lam: ch.place(np.full_like(np.atleast_1d(np.asarray(lam, float)), TAU), lam), 0.0, s_end - s0)
    p.info.update(scale=scale, s0=s0)
    return p


def branch_fold(p, pts_ref):
    """Reference-frame branch pattern points -> 3D points on the branch CONTACT surface."""
    phi, lam = p.chart.inverse(pts_ref)
    return phi, p.joint.branch_point(phi, p.s_end - lam)


def header_with_opening(R, R_dev, length, station, r, beta_deg, delta):
    j = Joint3D(R, r, beta_deg, delta, station)
    scale = max(TAU * R_dev, length, R)
    # seam at gamma = 0 : directrix starts opposite the branch side
    ch = CylChart(j.ORIGIN, R_dev, -j.T, -j.o, j.A, scale)
    p = RefPattern('header_shell_with_opening')
    p.joint, p.chart, p.length = j, ch, length

    def opening(phi):
        x, g = j.header_coords(j.curve3d(phi))
        return ch.place(g, x)

    p.add('end0', lambda g: ch.place(g, 0.0), 0.0, TAU)
    p.add('end1', lambda g: ch.place(g, length), 0.0, TAU)
    p.add('seam0', lambda x: ch.place(np.zeros_like(np.atleast_1d(np.asarray(x, float))), x), 0.0, length)
    p.add('seam1', lambda x: ch.place(np.full_like(np.atleast_1d(np.asarray(x, float)), TAU), x), 0.0, length)
    p.add('opening', opening, 0.0, TAU)
    p.info.update(scale=scale)
    return p


def header_fold(p, pts_ref):
    """Reference-frame header plate points -> 3D points on the header CONTACT surface."""
    g, x = p.chart.inverse(pts_ref)
    return x, g, p.joint.header_point(x, g)
