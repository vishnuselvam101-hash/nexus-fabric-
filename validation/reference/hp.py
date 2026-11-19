"""High-precision tier (mpmath, default 40 significant digits).

Independent of the float tier in two ways: arbitrary-precision arithmetic, and
different formulations where one exists (rulings of a cone are concurrent, so
its development is the arc length of the rulings' spherical image; the float
tier never uses the apex).  All quantities are derived from 3D vectors.
"""
import mpmath as mp

mp.mp.dps = 40


def V(*c):
    return tuple(mp.mpf(x) for x in c)


def add(a, b): return tuple(x + y for x, y in zip(a, b))
def sub(a, b): return tuple(x - y for x, y in zip(a, b))
def mul(k, a): return tuple(k * x for x in a)
def dot(a, b): return sum(x * y for x, y in zip(a, b))
def cross(a, b): return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
def norm(a): return mp.sqrt(dot(a, a))
def unit(a): return mul(1 / norm(a), a)


def M(x):
    """Exact conversion of a binary64 input (the value the product actually receives)."""
    return mp.mpf(x)


TAU = 2 * mp.pi


# ------------------------------------------------------------------ cylinder
def cylinder(r_dev, length):
    r, L = M(r_dev), M(length)
    return {'developed_width': TAU * r, 'length': L, 'area': TAU * r * L}


# ------------------------------------------------------------------ cones
class Cone:
    """General (oblique) circular cone frustum: C1 radius r1 at z=0, C2 radius r2 at z=h offset e along x."""

    def __init__(self, r1, r2, h, e=0.0):
        self.r1, self.r2, self.h, self.e = M(r1), M(r2), M(h), M(e)
        lam = self.r1 / (self.r1 - self.r2)         # where the ruling radius r1 + lam (r2 - r1) vanishes
        self.apex = add(self.C1(0), mul(lam, sub(self.C2(0), self.C1(0))))

    def C1(self, t): return (self.r1 * mp.cos(t), self.r1 * mp.sin(t), mp.mpf(0))
    def C2(self, t): return (self.e + self.r2 * mp.cos(t), self.r2 * mp.sin(t), self.h)
    def dC1(self, t): return (-self.r1 * mp.sin(t), self.r1 * mp.cos(t), mp.mpf(0))
    def dC2(self, t): return (-self.r2 * mp.sin(t), self.r2 * mp.cos(t), mp.mpf(0))

    def rho(self, t):
        return norm(sub(self.C1(t), self.apex))

    def dpsi(self, t):
        """Angular speed of the ruling direction = speed of the unit vector w/|w| on the unit sphere."""
        w = sub(self.C1(t), self.apex)
        return norm(cross(w, self.dC1(t))) / dot(w, w)

    def psi(self, t0, t1):
        return mp.quad(self.dpsi, mp.linspace(t0, t1, 9))

    def psi_total(self):
        return 2 * self.psi(0, mp.pi) if True else None

    def psi_table(self, n):
        """psi at t_i = i*2pi/n, i = 0..n (cumulative Gauss-Legendre quadrature per panel)."""
        out, acc = [mp.mpf(0)], mp.mpf(0)
        for i in range(n):
            acc += mp.quad(self.dpsi, [TAU * i / n, TAU * (i + 1) / n])
            out.append(acc)
        return out

    def area_surface(self):
        """Lateral area as the 3D surface integral of |S_t x S_lam| (no development involved)."""
        def f(t):
            d = sub(self.C2(t), self.C1(t))
            # |((1-lam) C1' + lam C2') x d| integrated over lam in [0,1]; C1' || C2' so the integrand is linear in lam
            a0 = norm(cross(self.dC1(t), d))
            a1 = norm(cross(self.dC2(t), d))
            return (a0 + a1) / 2
        return mp.quad(f, mp.linspace(0, TAU, 9))

    def area_polar(self):
        k = self.r2 / self.r1
        return mp.quad(lambda t: self.rho(t) ** 2 * self.dpsi(t) / 2, mp.linspace(0, TAU, 9)) * (1 - k * k)

    def slant(self, t=0):
        return norm(sub(self.C2(t), self.C1(t)))

    def pattern_point(self, t, psi, small=False):
        """Pattern point in the frame: apex at the origin, seam ruling (t=0) on +x, psi counter-clockwise."""
        rho = self.rho(t) * (self.r2 / self.r1 if small else 1)
        return (rho * mp.cos(psi), rho * mp.sin(psi))


def right_cone(r1, r2, h):
    c = Cone(r1, r2, h, 0.0)
    L1 = c.rho(0)
    L2 = L1 * c.r2 / c.r1
    phi = TAU * c.r1 / L1                       # the rim (length 2 pi r1) develops to an arc of radius L1
    return {'slant_length': c.slant(), 'sin_half_angle': c.r1 / L1, 'apex_radius_large': L1, 'apex_radius_small': L2,
            'sector_angle': phi, 'sector_angle_spherical': c.psi(0, TAU), 'area': c.area_surface(), 'area_sector': phi * (L1 ** 2 - L2 ** 2) / 2,
            'chord_large_arc': 2 * L1 * mp.sin(phi / 2)}


def eccentric(r1, r2, h, e):
    c = Cone(r1, r2, h, e)
    ps = c.psi(0, mp.pi)
    return {'developed_angle': c.psi(0, TAU), 'psi_180': ps, 'rho_at_0': c.rho(0), 'rho_at_180': c.rho(mp.pi), 'rho_at_90': c.rho(mp.pi / 2),
            'area': c.area_surface(), 'area_polar': c.area_polar(), 'seam_length': c.slant(0)}


def eccentric_apexfree_chord(r1, r2, h, e, dps=25):
    """Developed chord between the large-end points at 0 and 180 deg by integrating the geodesic
    curvature of the large circle with mpmath's Taylor ODE solver (no apex anywhere)."""
    with mp.workdps(dps):
        c = Cone(r1, r2, h, e)

        def rhs(t, y):
            dc = c.dC1(t)
            sp = norm(dc)
            T = mul(1 / sp, dc)
            d = sub(c.C2(t), c.C1(t))
            n = unit(cross(dc, d))
            if dot(n, (mp.cos(t), mp.sin(t), 0)) < 0:
                n = mul(-1, n)
            l = cross(n, T)
            ddc = (-c.r1 * mp.cos(t), -c.r1 * mp.sin(t), mp.mpf(0))
            return [dot(ddc, l) / sp, sp * mp.cos(y[0]), sp * mp.sin(y[0])]

        f = mp.odefun(rhs, 0, [0, 0, 0])
        y = f(mp.pi)
        return mp.sqrt(y[1] ** 2 + y[2] ** 2)


# ------------------------------------------------------------------ cylinder / cylinder joint
class Joint:
    """Header (axis a through the origin, radius R) and branch (axis direction b at angle beta to a,
    offset delta along o = (b x a)/|b x a|, radius r).  Frame chosen here: a = z, transverse t = x."""

    def __init__(self, R, r, beta_deg, delta):
        self.R, self.r, self.delta = M(R), M(r), M(delta)
        self.beta = M(beta_deg) * mp.pi / 180
        self.a = V(0, 0, 1)
        self.t = V(1, 0, 0)
        self.b = add(mul(mp.cos(self.beta), self.a), mul(mp.sin(self.beta), self.t))
        self.o = unit(cross(self.b, self.a))
        self.e2 = cross(self.b, self.o)
        self.Q = mul(self.delta, self.o)

    def perp(self, v):
        return sub(v, mul(dot(v, self.a), self.a))

    def s(self, phi):
        P0 = add(self.Q, mul(self.r, add(mul(mp.cos(phi), self.o), mul(mp.sin(phi), self.e2))))
        w0, w1 = self.perp(P0), self.perp(self.b)
        A, B, C = dot(w1, w1), dot(w0, w1), dot(w0, w0) - self.R ** 2
        return (-B + mp.sqrt(B * B - A * C)) / A        # the far root along +b: the side the branch arrives from

    def X(self, phi):
        s = self.s(phi)
        return add(add(self.Q, mul(s, self.b)), mul(self.r, add(mul(mp.cos(phi), self.o), mul(mp.sin(phi), self.e2))))

    def plate(self, phi, R_dev, station=0):
        """Header plate coordinates (station along a, arc from the seam opposite t, toward o first... i.e. angle from t toward o, plus pi)."""
        X = self.X(phi)
        ang = mp.atan2(dot(X, self.o), dot(X, self.t))
        return (M(station) + dot(X, self.a), M(R_dev) * (ang + mp.pi))

    def length3d(self):
        return mp.quad(lambda p: norm(mp.diff(lambda q: mp.matrix(self.X(q)), p)), mp.linspace(0, TAU, 17))


def _curve_length(f):
    return mp.quad(lambda p: mp.sqrt(sum(c ** 2 for c in [mp.diff(lambda q: f(q)[k], p) for k in (0, 1)])), mp.linspace(0, TAU, 33))


def joint_quantities(R, r, beta_deg, delta, R_dev=None, r_dev=None, s_end=None, dps=25):
    """Integral quantities of the saddle / opening (reduced working precision: numerical differentiation inside quadrature)."""
    with mp.workdps(dps):
        j = Joint(R, r, beta_deg, delta)
        R_dev = M(R if R_dev is None else R_dev)
        r_dev = M(r if r_dev is None else r_dev)
        out = {'s_at_0': j.s(0), 's_at_90': j.s(mp.pi / 2), 's_at_180': j.s(mp.pi), 's_at_270': j.s(3 * mp.pi / 2)}
        op = lambda p: j.plate(p, R_dev)
        out['opening_perimeter'] = _curve_length(op)
        # signed area by Green's theorem; the sign depends on the direction of phi, so the magnitude is reported
        out['opening_area'] = abs(mp.quad(lambda p: (op(p)[0] * mp.diff(lambda q: op(q)[1], p) - op(p)[1] * mp.diff(lambda q: op(q)[0], p)) / 2, mp.linspace(0, TAU, 33)))
        out['saddle_developed_length'] = _curve_length(lambda p: (r_dev * p, j.s(p)))
        out['mean_s'] = mp.quad(j.s, mp.linspace(0, TAU, 33)) / TAU
        if s_end is not None:
            out['branch_pattern_area'] = TAU * r_dev * (M(s_end) - out['mean_s'])
        return out
