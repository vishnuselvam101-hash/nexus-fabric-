"""Generic development of developable ruled surfaces  S(t, lam) = C(t) + lam * D(t).

Method (first principles, no family-specific closed form):
  * the directrix C is developed into the plane by integrating its geodesic
    curvature:  dTheta/dt = C'' . (n x T) / |C'|,  dc/dt = |C'| (cos Theta, sin Theta),
    with T the unit tangent and n the unit surface normal chosen to point to the
    OUTSIDE of the part.  The pattern is therefore seen from the outside:
    a rotation from T toward (n x T) on the surface is counter-clockwise in the plane.
  * rulings are geodesics, so they develop to straight lines; the ruling
    direction keeps its angle with the developed tangent.
Integrator: scipy DOP853 with dense output (float tier).  The surface classes
are plain numpy; nothing here knows about cylinders or cones.
"""
import numpy as np
from scipy.integrate import solve_ivp


def _unit(v):
    return v / np.linalg.norm(v, axis=-1, keepdims=True)


class Circle3D:
    def __init__(self, centre, radius, e1, e2):
        self.c = np.asarray(centre, float)
        self.r = float(radius)
        self.e1 = np.asarray(e1, float)
        self.e2 = np.asarray(e2, float)

    def p(self, t):
        t = np.atleast_1d(np.asarray(t, float))[:, None]
        return self.c + self.r * (np.cos(t) * self.e1 + np.sin(t) * self.e2)

    def d1(self, t):
        t = np.atleast_1d(np.asarray(t, float))[:, None]
        return self.r * (-np.sin(t) * self.e1 + np.cos(t) * self.e2)

    def d2(self, t):
        t = np.atleast_1d(np.asarray(t, float))[:, None]
        return -self.r * (np.cos(t) * self.e1 + np.sin(t) * self.e2)

    def radial(self, t):
        t = np.atleast_1d(np.asarray(t, float))[:, None]
        return np.cos(t) * self.e1 + np.sin(t) * self.e2


class RuledSurface:
    """Directrix = a 3D circle; ruling given by a callable D(t) -> (N,3)."""

    def __init__(self, circle, ruling):
        self.circle = circle
        self.ruling = ruling

    def C(self, t):
        return self.circle.p(t)

    def dC(self, t):
        return self.circle.d1(t)

    def ddC(self, t):
        return self.circle.d2(t)

    def D(self, t):
        return self.ruling(np.atleast_1d(np.asarray(t, float)))

    def outward(self, t):
        return self.circle.radial(t)

    def point(self, t, lam):
        lam = np.atleast_1d(np.asarray(lam, float))[:, None]
        return self.C(t) + lam * self.D(t)

    def frame(self, t):
        dc = self.dC(t)
        speed = np.linalg.norm(dc, axis=-1)
        T = dc / speed[:, None]
        d = self.D(t)
        n = _unit(np.cross(dc, d))
        sgn = np.sign(np.einsum('ij,ij->i', n, self.outward(t)))
        n = n * sgn[:, None]
        l = np.cross(n, T)
        return T, n, l, speed, d

    def developability_defect(self, t, h=1e-5):
        """|det[C', D, D']| / (|C'| |D|^2): zero for a developable surface."""
        t = np.atleast_1d(np.asarray(t, float))
        dD = (self.D(t + h) - self.D(t - h)) / (2 * h)
        dc, d = self.dC(t), self.D(t)
        det = np.einsum('ij,ij->i', dc, np.cross(d, dD))
        return np.abs(det) / (np.linalg.norm(dc, axis=-1) * np.linalg.norm(d, axis=-1) ** 2)


class Development:
    """Planar development of a RuledSurface over t in [t0, t1]."""

    def __init__(self, surf, t0, t1, scale=1.0, rtol=1e-13):
        self.surf, self.t0, self.t1 = surf, t0, t1

        def rhs(t, y):
            T, n, l, speed, d = surf.frame(t)
            kg_speed = float(np.dot(surf.ddC(t)[0], l[0]) / speed[0])
            return [kg_speed, speed[0] * np.cos(y[0]), speed[0] * np.sin(y[0])]

        sol = solve_ivp(rhs, (t0, t1), [0.0, 0.0, 0.0], method='DOP853', rtol=rtol,
                        atol=[1e-14, 1e-14 * scale, 1e-14 * scale], dense_output=True, max_step=(t1 - t0) / 64)
        if not sol.success:
            raise RuntimeError('development ODE failed: ' + sol.message)
        self._sol = sol.sol

    def base(self, t):
        y = self._sol(np.atleast_1d(np.asarray(t, float)))
        return y[0], np.stack([y[1], y[2]], axis=-1)

    def place(self, t, lam):
        """Pattern coordinates of S(t, lam)."""
        t = np.atleast_1d(np.asarray(t, float))
        lam = np.broadcast_to(np.asarray(lam, float), t.shape)
        th, c = self.base(t)
        T, n, l, speed, d = self.surf.frame(t)
        a = np.einsum('ij,ij->i', d, T)
        b = np.einsum('ij,ij->i', d, l)
        t2 = np.stack([np.cos(th), np.sin(th)], axis=-1)
        n2 = np.stack([-np.sin(th), np.cos(th)], axis=-1)
        return c + lam[:, None] * (a[:, None] * t2 + b[:, None] * n2)


def unroll_strip(P, Q):
    """Triangulation unrolling (third, deliberately crude method).

    P, Q: (N,3) corresponding samples of the two boundary curves of a ruled strip.
    The strip P0 Q0 P1 Q1 ... is split into triangles (Pi,Qi,Pi+1), (Pi+1,Qi,Qi+1)
    which are laid flat one after another, each keeping its three edge lengths.
    The side from which the result is seen is not specified: use it only for
    quantities invariant under rigid motion and mirroring (distances, angles).
    Returns the flat positions (p2, q2).  Converges at second order in the facet size.
    """
    P, Q = np.asarray(P, float), np.asarray(Q, float)
    n = len(P)
    p2 = np.zeros((n, 2))
    q2 = np.zeros((n, 2))
    q2[0] = [0.0, np.linalg.norm(Q[0] - P[0])]

    def third(a2, b2, da, db, left):
        # point at distance da from a2 and db from b2, on the left/right of a2->b2
        ab = b2 - a2
        L = np.linalg.norm(ab)
        x = (da * da - db * db + L * L) / (2 * L)
        y2 = max(da * da - x * x, 0.0)
        y = np.sqrt(y2)
        ex = ab / L
        ey = np.array([-ex[1], ex[0]])
        return a2 + x * ex + (y if left else -y) * ey

    for i in range(n - 1):
        # triangle (Pi, Qi, Pi+1): Pi+1 lies to the right of the directed edge Pi->Qi
        p2[i + 1] = third(p2[i], q2[i], np.linalg.norm(P[i + 1] - P[i]), np.linalg.norm(P[i + 1] - Q[i]), left=False)
        # triangle (Pi+1, Qi, Qi+1): Qi+1 lies to the left of the directed edge Pi+1->Qi  ... i.e. right of Qi->Pi+1
        q2[i + 1] = third(q2[i], p2[i + 1], np.linalg.norm(Q[i + 1] - Q[i]), np.linalg.norm(Q[i + 1] - P[i + 1]), left=True)
    return p2, q2


def fit_rigid(src, dst, proper=True):
    """Least-squares rigid map  x -> R x + t  from src to dst (2D).
    proper=True forces det R = +1 (no mirroring); proper=False forces det R = -1.
    Returns (R, t, max residual)."""
    src, dst = np.asarray(src, float), np.asarray(dst, float)
    sc, dc = src.mean(0), dst.mean(0)
    H = (src - sc).T @ (dst - dc)
    U, S, Vt = np.linalg.svd(H)
    d = np.sign(np.linalg.det(Vt.T @ U.T)) or 1.0
    Dm = np.diag([1.0, d if proper else -d])
    R = Vt.T @ Dm @ U.T
    t = dc - R @ sc
    res = np.max(np.linalg.norm((src @ R.T + t) - dst, axis=1))
    return R, t, float(res)


def dist_pts_to_param_curve(f, t0, t1, pts, coarse=2049, iters=34):
    """Distance from each point to the parametric curve f(t), t in [t0,t1]
    (coarse nearest sample, then golden-section refinement on the neighbouring
    interval; f must accept an array of parameters)."""
    pts = np.asarray(pts, float)
    ts = np.linspace(t0, t1, coarse)
    P = f(ts)
    d2 = ((pts[:, None, :] - P[None, :, :]) ** 2).sum(-1)
    k = d2.argmin(1)
    h = (t1 - t0) / (coarse - 1)
    a = np.clip(ts[k] - h, t0, t1)
    b = np.clip(ts[k] + h, t0, t1)
    g = (np.sqrt(5.0) - 1) / 2
    c = b - g * (b - a)
    d = a + g * (b - a)

    def dist(t):
        return np.linalg.norm(f(t) - pts, axis=1)

    fc, fd = dist(c), dist(d)
    for _ in range(iters):
        m = fc < fd
        b = np.where(m, d, b)
        a = np.where(m, a, c)
        c2 = b - g * (b - a)
        d2_ = a + g * (b - a)
        fc_new, fd_new = dist(c2), dist(d2_)
        c, d, fc, fd = c2, d2_, fc_new, fd_new
    tm = 0.5 * (a + b)
    return np.minimum(dist(tm), np.sqrt(d2.min(1))), tm
