# Exact residual check, independent of the validation harness's nearest-point search:
# every header-opening vertex, folded back onto the header cylinder, must lie on the branch cylinder (distance to the branch axis = r).
import sys, math, json, numpy as np
sys.path.insert(0, 'validation')
import run_all
from scipy.optimize import minimize_scalar
from checks.client import Client, SpawnedServer
server = SpawnedServer(sys.argv[1], 8856)
client = Client(server.url if hasattr(server,'url') else 'http://127.0.0.1:8856').connect(setup_token=server.token)
run_all._client = client
import atexit; atexit.register(server.stop)
cases = [
 dict(family='header_with_branch', header_radius=150.0, header_dev_radius=150.0, header_length=450.0, branch_station=225.0, branch_radius=75.0, branch_dev_radius=75.0, beta_deg=90, delta=74.9997, s_end=300.0, surface_reference='outside'),
 dict(family='header_with_branch', header_radius=150.0, header_dev_radius=150.0, header_length=725.0, branch_station=213.0, branch_radius=149.999835, branch_dev_radius=149.999835, beta_deg=45, delta=0.0, s_end=513.0, surface_reference='outside'),
 dict(family='header_with_branch', header_radius=150.0, header_dev_radius=150.0, header_length=538.0, branch_station=146.0, branch_radius=75.0, branch_dev_radius=75.0, beta_deg=45, delta=73.5, s_end=422.0, surface_reference='outside'),
]
for rq in cases:
    job = run_all.submit(rq)
    h = job.parts['header']
    pts = np.asarray(h.curve('branch_opening').pts, float)
    R, r, b, d, st, L = rq['header_radius'], rq['branch_radius'], math.radians(rq['beta_deg']), rq['delta'], rq['branch_station'], rq['header_length']
    best = None
    for xflip in (0, 1):
        for sgn in (1, -1):
            x = (L - pts[:, 0]) if xflip else pts[:, 0]
            def res(th0):
                th = sgn * pts[:, 1] / R + th0
                P = np.stack([x, R * np.sin(th), R * np.cos(th)], 1)
                A = np.array([st, d, 0.0]); u = np.array([math.cos(b), 0.0, math.sin(b)])
                w = P - A; perp = w - np.outer(w @ u, u)
                return np.abs(np.linalg.norm(perp, axis=1) - r)
            # coarse scan then refine the seam angle
            g = np.linspace(-math.pi, math.pi, 7201)
            v = [res(t).max() for t in g]; i = int(np.argmin(v))
            o = minimize_scalar(lambda t: res(t).max(), bounds=(g[max(i-1,0)], g[min(i+1,len(g)-1)]), method='bounded', options={'xatol': 1e-14})
            cands = [o.x] + [k*math.pi/2 for k in (-2,-1,0,1,2)]
            for tt in cands:
                rr = res(tt)
                if best is None or rr.max() < best[0]: best = (rr.max(), xflip, sgn, tt, rr)
    mx, xflip, sgn, th0, rr = best
    seg = np.linalg.norm(np.diff(pts, axis=0), axis=1)
    print(f"beta={rq['beta_deg']} delta={d} r={r}: vertices={len(pts)}  max |dist-to-branch-axis - r| = {mx:.3e} mm  (seam angle {th0:.9f} rad, xflip={xflip}, sense={sgn});  longest segment {seg.max():.4f} mm, shortest {seg.min():.3e} mm;  eps_num={h.eps_num}")
