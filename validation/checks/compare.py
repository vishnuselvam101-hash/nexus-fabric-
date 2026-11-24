"""Comparison of a product job with the independent reference.

For every part:
  1. DXF round trip (two readers, entity geometry / layers / closure / orientation / units vs the JSON result);
  2. frame mapping: the reference pattern (seen from outside, arbitrary position) is mapped onto the product
     pattern by an explicit rigid motion fitted to semantic corner points, once WITHOUT mirroring (product drawn
     as seen from outside) and once WITH mirroring (drawn as seen from inside), for every admissible labelling
     of the seam edges / plate ends.  Outline and hole are then compared point by point in both directions;
  3. fold-back: product pattern points are carried through the inverse mapping and the reference's inverse
     development into 3D, and tested against the 3D target surfaces by point-to-axis distances;
  4. for assemblies: the folded header opening and the folded branch saddle must coincide in 3D.
"""
import hashlib
import math
import numpy as np
import shapely

from reference import families as RF
from reference.ruled import fit_rigid, dist_pts_to_param_curve

TAU = 2 * math.pi
EXPECTED_LAYERS = {'outline': 'CUT_OUTER', 'hole': 'CUT_INNER'}


def rel_tol(scale):
    """Agreement tolerance for point positions: far above float noise of either side, far below any fabrication
    relevance (2e-9 of the largest coordinate, never less than 1e-12 mm)."""
    return max(2e-9 * scale, 1e-12)


# ------------------------------------------------------------------ polylines (exact measurements, no search)
# Scale relation: lengths in a request are multiplied by k; angles, family names and flags are not.
LENGTH_KEYS = ['r_dev', 'length', 'r1', 'r2', 'h', 'e', 'header_radius', 'header_dev_radius', 'header_length',
               'branch_station', 'branch_radius', 'branch_dev_radius', 'delta', 's_end']


def scale_request(req, k):
    return {kk: (v * k if kk in LENGTH_KEYS else v) for kk, v in req.items()}


def _finite(a):
    a = np.asarray(a, float)
    if not np.all(np.isfinite(a)):
        raise RuntimeError('reference curve is not finite where the comparison needs it')
    return a


def adaptive_polyline(f, t0, t1, tc, n0=2049, max_pts=2000000):
    """Vertices of the exact curve f(t), t in [t0, t1], refined in the parameter until every chord lies within tc of
    the curve (deviation tested at 1/4, 1/2 and 3/4 of each chord).  The product is then measured against this dense
    polyline vertex-to-segment, which avoids a nearest-point search over a coarse uniform sample."""
    t = np.linspace(t0, t1, n0)
    X = _finite(f(t))
    # only the segments created by the last split are tested again; accepted segments are kept as they are
    ta, tb, Xa, Xb = t[:-1], t[1:], X[:-1], X[1:]
    kept_t, kept_X, n_kept = [], [], 0
    for _ in range(80):
        h = tb - ta
        T = np.stack([ta + 0.25 * h, ta + 0.5 * h, ta + 0.75 * h])
        Xt = _finite(f(T.ravel())).reshape(3, len(ta), -1)
        v = Xb - Xa
        L2 = np.maximum((v * v).sum(1), 1e-300)
        u = np.clip(((Xt - Xa) * v).sum(-1) / L2, 0.0, 1.0)
        dev = np.linalg.norm(Xt - (Xa + u[..., None] * v), axis=-1).max(0)
        ok = dev <= tc
        kept_t.append(ta[ok])
        kept_X.append(Xa[ok])
        n_kept += int(ok.sum())
        bad = ~ok
        if not bad.any():
            break
        if n_kept + 2 * int(bad.sum()) > max_pts:
            raise RuntimeError(f'reference curve needs more than {max_pts} points for a chord error of {tc:.3e} mm')
        tm, Xm = T[1][bad], Xt[1][bad]
        ta, tb = np.concatenate([ta[bad], tm]), np.concatenate([tm, tb[bad]])
        Xa, Xb = np.concatenate([Xa[bad], Xm]), np.concatenate([Xm, Xb[bad]])
    else:
        raise RuntimeError('reference curve did not reach the chord tolerance')
    tk = np.concatenate(kept_t + [t[-1:]])
    Xk = np.concatenate(kept_X + [X[-1:]])
    return Xk[np.argsort(tk, kind='stable')]


def _seg_tree(Q):
    """Spatial index over the segments of the polyline Q."""
    Q = np.asarray(Q, float)
    return shapely.STRtree(shapely.linestrings(np.stack([Q[:-1], Q[1:]], axis=1)))


def _dist_to_tree(P, tree):
    """Exact distance from each point of P to the polyline behind tree (vertex to segment)."""
    P = np.asarray(P, float)
    idx, d = tree.query_nearest(shapely.points(P), return_distance=True)
    out = np.full(len(P), np.inf)
    out[idx[0]] = d
    return out


def two_sided(P, Q):
    """Two-sided polyline distance (mm): the largest vertex-to-segment distance from P to Q and from Q to P."""
    return max(float(_dist_to_tree(P, _seg_tree(Q)).max()), float(_dist_to_tree(Q, _seg_tree(P)).max()))


def _exact_pair(ref, curve, tol, onsurf, on_tol):
    """Exact test for one curve of the reference: its polyline is sampled to a chord error of tol/10 (reference
    frame); onsurf(points) is the residual of the folded product points to the other surface, which must be <= on_tol."""
    rc = ref.curves[curve]
    X = adaptive_polyline(rc.f, rc.t0, rc.t1, tol / 10.0)
    return (X, _seg_tree(X), onsurf, on_tol)


# ------------------------------------------------------------------ 1. DXF round trip
def outline_checks(part, issues, tol, m):
    """Checks that need only the outline and the openings: the drawn coordinates that the JSON states exactly, so they also
    run on a withheld part (no file).  Closure and orientation of the outer loop, closure and orientation of the openings,
    polygon validity with an independent geometry kernel."""
    # closure and orientation of the outer loop
    gaps = []
    for a, b in zip(part.outline, part.outline[1:] + part.outline[:1]):
        gaps.append(min(np.linalg.norm(p - q) for p in (a.start, a.end) for q in (b.start, b.end)))
    m['outline_max_gap'] = float(max(gaps))
    if max(gaps) > max(tol * 1e3, 1e-12):
        issues.append(f'{part.name}: outline not closed, gap {max(gaps):.3e}')
    sa = part.loop_area(part.outline)
    m['outline_signed_area'] = float(sa)
    if not sa > 0:
        issues.append(f'{part.name}: outer profile is not counter-clockwise (signed area {sa:.6g})')
    for h in part.holes:
        if np.linalg.norm(h.start - h.end) > tol:
            issues.append(f'{part.name}/{h.name}: opening polyline not closed')
        if not h.area_term() < 0:
            issues.append(f'{part.name}/{h.name}: opening is not clockwise')
    # polygon validity with an independent geometry kernel
    try:
        segs = []
        for c, start in _chain(part.outline):
            d = c.dense(513)
            if np.linalg.norm(d[0] - start) > np.linalg.norm(d[-1] - start):
                d = d[::-1]
            if segs:
                d = d[1:]            # the shared corner is taken once (corner gaps are tested separately above)
            segs.append(d)
        ring = np.concatenate(segs)[:-1]
        poly = shapely.Polygon(ring, [h.pts for h in part.holes])
        m['polygon_valid'] = bool(poly.is_valid)
        if not poly.is_valid:
            issues.append(f'{part.name}: outline/opening do not form a valid polygon ({shapely.is_valid_reason(poly)})')
        if part.holes:
            outer = shapely.Polygon(ring)
            hole = shapely.Polygon(part.holes[0].pts)
            m['hole_edge_clearance'] = float(outer.exterior.distance(hole))
            if not outer.contains(hole):
                issues.append(f'{part.name}: opening is not inside the plate outline')
    except Exception as ex:  # degenerate geometry
        issues.append(f'{part.name}: polygon construction failed: {ex}')


def json_geometry(part, issues):
    """The geometry-only checks of a withheld part, whose file is not served (the round trip cannot run): finite coordinates,
    the length each curve states, and outline_checks.  The same tolerances as dxf_roundtrip."""
    scale = max(part.extent(), 1e-300)
    tol = 1e-12 * scale
    jc = part.view['curves'] + part.view['holes']
    m = {'entities': len(part.ents)}
    for e, c in zip(part.ents, jc):
        if abs(e.length() - c['length']) > 1e-9 * scale:
            issues.append(f"{part.name}/{c['name']}: length JSON {c['length']} vs drawn {e.length()}")
        if not np.all(np.isfinite(e.pts)):
            issues.append(f"{part.name}/{c['name']}: non-finite coordinates in the result")
    outline_checks(part, issues, tol, m)
    return m


def dxf_roundtrip(part, issues):
    v = part.view
    scale = max(part.extent(), 1e-300)
    tol = 1e-12 * scale
    jc = v['curves'] + v['holes']
    m = {'entities': len(part.ents)}
    if hashlib.sha256(part.dxf_text.encode()).hexdigest() != v.get('dxf_sha256'):
        issues.append(f'{part.name}: served DXF does not hash to dxf_sha256 of the result')
    # entities that are not cut geometry: only the non-cutting NOT-RELEASED marking text is tolerated
    m['non_cut_entities'] = [f'{e.kind}@{e.layer}' for e in part.other]
    for e in part.other:
        if not (e.kind == 'text' and e.layer == 'NF_NOT_RELEASED'):
            issues.append(f'{part.name}: DXF contains {e.kind} on layer {e.layer}: geometry outside the CUT layers / an unexpected entity')
    for e in part.ents:
        if e.kind not in ('line', 'arc', 'polyline'):
            issues.append(f'{part.name}: DXF contains a {e.kind} entity on cut layer {e.layer}')
    if len(part.all) != len(part.all_min) or any(a.kind != b.kind or a.layer != b.layer for a, b in zip(part.all, part.all_min)):
        issues.append(f'{part.name}: ezdxf and the minimal reader disagree on the entity list of the DXF')
    if len(part.ents) != len(jc):
        issues.append(f'{part.name}: DXF has {len(part.ents)} cut entities, JSON has {len(jc)} curves')
        return m
    # the two readers must see the same file
    if len(part.ents_min) != len(part.ents) or any(a.kind != b.kind or a.layer != b.layer or a.pts.shape != b.pts.shape or np.abs(a.pts - b.pts).max() > 0 for a, b in zip(part.ents, part.ents_min)):
        issues.append(f'{part.name}: ezdxf and the minimal reader disagree on the DXF content')
    for i, (e, c) in enumerate(zip(part.ents, jc)):
        role = 'outline' if i < part.n_outline else 'hole'
        if e.kind != c['kind']:
            issues.append(f"{part.name}/{c['name']}: DXF entity {e.kind} but JSON kind {c['kind']}")
            continue
        if e.layer != c['layer'] or e.layer != EXPECTED_LAYERS[role]:
            issues.append(f"{part.name}/{c['name']}: layer DXF={e.layer} JSON={c['layer']} expected {EXPECTED_LAYERS[role]}")
        jp = np.asarray(c['points'], float)
        if e.kind == 'arc':
            d = e.dist(jp).max()
            dend = max(np.linalg.norm(jp - e.start, axis=1).min(), np.linalg.norm(jp - e.end, axis=1).min())   # the JSON may drop a closing point that repeats the first
            if d > 1e-9 * scale or dend > 1e-9 * scale:
                issues.append(f"{part.name}/{c['name']}: JSON arc points off the DXF arc by {max(d, dend):.3e}")
            m.setdefault('json_arc_sagitta', 0.0)
            # how far the JSON display polyline is from the true arc, and whether the result says so
            mid = 0.5 * (jp[:-1] + jp[1:])
            sag = float(e.dist(mid).max())
            m['json_arc_sagitta'] = max(m['json_arc_sagitta'], sag)
            labelled = c.get('points_exact') is False and isinstance(c.get('points_max_deviation'), (int, float)) and c['points_max_deviation'] * (1 + 1e-6) + 1e-12 * scale >= sag
            m['json_arc_labelled'] = bool(m.get('json_arc_labelled', True) and labelled)
        else:
            # every JSON point must be a DXF vertex bit-for-bit (the JSON may drop a repeated closing point)
            dv = set(map(tuple, e.pts.tolist()))
            missing = [p for p in jp.tolist() if tuple(p) not in dv]
            if missing or abs(len(e.pts) - len(jp)) > 1:
                issues.append(f"{part.name}/{c['name']}: JSON points differ from DXF vertices ({len(missing)} not found; counts {len(jp)} vs {len(e.pts)})")
        if abs(e.length() - c['length']) > 1e-9 * scale:
            issues.append(f"{part.name}/{c['name']}: length JSON {c['length']} vs DXF {e.length()}")
        if not np.all(np.isfinite(e.pts)):
            issues.append(f"{part.name}/{c['name']}: non-finite coordinates in DXF")
    outline_checks(part, issues, tol, m)
    # header data of the file
    mm = part.meta_min
    m['units_declared'] = ('comment' if any('millimetre' in c.lower() for c in mm['comment']) else 'none') + ('+text' if any('MILLIMETRE' in getattr(e, 'text', '').upper() for e in part.other) else '')
    m['lunits'], m['aunits'], m['angdir'] = mm.get('lunits'), mm.get('aunits'), mm.get('angdir')
    if mm.get('aunits') not in (None, '0') or mm.get('angdir') not in (None, '0'):
        issues.append(f"{part.name}: DXF declares $AUNITS={mm.get('aunits')} $ANGDIR={mm.get('angdir')}; arcs are read as decimal degrees, counter-clockwise")
    m['insunits'] = mm['insunits']
    if not mm['eof']:
        issues.append(f'{part.name}: DXF does not end with EOF')
    bb = part.measures()
    if 'extmin' in mm and 'extmax' in mm:
        # The marking text is placed by the writer relative to its own box, so it must not be allowed to excuse a
        # wrong box.  Judged on the two sides the text does not touch: the left edge and the top edge of the cut
        # geometry (the text sits below the part and runs to the right; its width needs font metrics).
        has_text = any(e.kind == 'text' for e in part.other)
        if has_text:
            ext_err = max(abs(mm['extmin'][0] - bb['bbox_xmin']), abs(mm['extmax'][1] - bb['bbox_ymax']), max(bb['bbox_xmax'] - mm['extmax'][0], 0.0), max(mm['extmin'][1] - bb['bbox_ymin'], 0.0))
        else:
            ext_err = max(abs(mm['extmin'][0] - bb['bbox_xmin']), abs(mm['extmin'][1] - bb['bbox_ymin']), abs(mm['extmax'][0] - bb['bbox_xmax']), abs(mm['extmax'][1] - bb['bbox_ymax']))
        m['extents_header_error'] = float(ext_err)
    return m


def _chain(curves):
    prev = None
    out = []
    for i, c in enumerate(curves):
        if prev is None:
            nxt = curves[(i + 1) % len(curves)]
            d_end = min(np.linalg.norm(c.end - nxt.start), np.linalg.norm(c.end - nxt.end))
            d_start = min(np.linalg.norm(c.start - nxt.start), np.linalg.norm(c.start - nxt.end))
            start = c.end if d_start < d_end else c.start
        else:
            start = prev
        out.append((c, start))
        prev = c.end if np.linalg.norm(c.start - start) <= np.linalg.norm(c.end - start) else c.start
    return out


# ------------------------------------------------------------------ 2. frame mapping
class Hyp:
    def __init__(self, label, ref_pts, prod_pts, pairs, natural=True):
        self.label, self.ref_pts, self.prod_pts, self.pairs, self.natural = label, np.asarray(ref_pts), np.asarray(prod_pts), pairs, natural


def evaluate(ref, part, hyps, scale, ndense=6001, tol=None, exact=None):
    """Try every labelling hypothesis with and without mirroring.  Returns list of dicts.
    exact: {reference pair name: (polyline, tree, onsurf, on_tol)} for the pairs that have an exact test (header opening,
    branch saddle).  For those, product vertices are measured to the adaptively sampled reference curve and, folded back,
    must lie on the other surface within on_tol.  Every other pair keeps the nearest-point search.  The frame hypotheses
    and the anchor test are the same for all pairs."""
    tol = rel_tol(scale) if tol is None else tol
    exact = exact or {}
    out = []
    for h in hyps:
        for proper in (True, False):
            R, t, res = fit_rigid(h.ref_pts, h.prod_pts, proper)
            e = {'label': h.label, 'natural': h.natural, 'view': 'outside' if proper else 'inside', 'anchor_residual': res, 'R': R, 't': t, 'r2p': {}, 'p2r': {}, 'onsurf': {}}
            if res <= 1e-6 * scale:
                for rn, pn in h.pairs:
                    rc, pc = ref.curves[rn], part.curve(pn)
                    if pn in exact:
                        X, tree_ref, onsurf, _ = exact[pn]
                        pv = pc.vertices()
                        e['p2r'][pn] = float(_dist_to_tree((pv - t) @ R, tree_ref).max())          # product -> exact curve
                        e['r2p'][pn] = float(_dist_to_tree(X @ R.T + t, _seg_tree(pv)).max())      # curve -> product polyline
                        e['onsurf'][pn] = float(onsurf((pv - t) @ R))
                        continue
                    f = lambda tt, rc=rc: rc.f(tt) @ R.T + t
                    pts = f(np.linspace(rc.t0, rc.t1, ndense))
                    ok = np.all(np.isfinite(pts), axis=1)
                    e['r2p'][pn] = float(pc.dist(pts[ok]).max()) if ok.any() else float('nan')
                    d, _ = dist_pts_to_param_curve(f, rc.t0, rc.t1, pc.vertices())
                    e['p2r'][pn] = float(np.nanmax(d))
                e['vertex_dev'] = max(e['p2r'].values())
                e['chord_dev'] = max(e['r2p'].values())
                on_ok = all(e['onsurf'][pn] <= exact[pn][3] for pn in e['onsurf'])
                e['match'] = bool(res <= tol and e['vertex_dev'] <= tol and on_ok)
            else:
                e['match'] = False
            out.append(e)
    return out


def _to_ref(e, pts):
    return (np.asarray(pts, float) - e['t']) @ e['R']


def _pick(evals, view=None, natural_only=True):
    c = [e for e in evals if e['match'] and (view is None or e['view'] == view) and (e['natural'] or not natural_only)]
    return c[0] if c else None


def _summarise(evals, rep, part, scale, issues, tol=None):
    tol = rel_tol(scale) if tol is None else tol
    matched = [e for e in evals if e['match'] and e['natural']]
    rep['views_matching'] = sorted({e['view'] for e in matched})
    rep['other_labellings_matching'] = sorted({e['label'] + '/' + e['view'] for e in evals if e['match'] and not e['natural']})
    if not matched:
        cand = [e for e in evals if 'vertex_dev' in e]
        best = min(cand, key=lambda e: e['vertex_dev']) if cand else min(evals, key=lambda e: e['anchor_residual'])
        rep['best'] = {k: best.get(k) for k in ('label', 'view', 'anchor_residual', 'vertex_dev', 'chord_dev', 'p2r', 'r2p')}
        issues.append(f"{part.name}: no frame mapping reproduces the reference (best: {best['label']}/{best['view']}, anchor residual {best['anchor_residual']:.3e}, "
                      f"vertex deviation {best.get('vertex_dev', float('nan')):.3e}, tolerance {tol:.3e})")
        return None
    e = matched[0]
    rep['mapping'] = e['label']
    rep['anchor_residual'] = e['anchor_residual']
    rep['vertex_dev'] = e['vertex_dev']      # product vertices -> exact reference curve
    rep['chord_dev'] = e['chord_dev']        # exact reference curve -> product entity (true representation error)
    rep['p2r'], rep['r2p'] = e['p2r'], e['r2p']
    rep['eps_num_stated'], rep['eps_rep_stated'] = part.eps_num, part.eps_rep
    rep['eps_rep_ratio'] = e['chord_dev'] / part.eps_rep if part.eps_rep > 0 else (0.0 if e['chord_dev'] <= tol else float('inf'))
    return e


# ------------------------------------------------------------------ family comparisons
def cmp_cylinder(req, part, issues):
    ref = RF.cylinder_shell(req['r_dev'], req['length'])
    L = req['length']
    scale = max(part.extent(), ref.info['scale'])
    c = ref.curves
    ra = [c['end0'].f(0.0)[0], c['end0'].f(TAU)[0], c['end1'].f(TAU)[0], c['end1'].f(0.0)[0]]
    hyps = []
    for s0, s1 in (('seam_a', 'seam_b'), ('seam_b', 'seam_a')):
        pa = [part.corner(s0, 'bottom'), part.corner(s1, 'bottom'), part.corner(s1, 'top'), part.corner(s0, 'top')]
        hyps.append(Hyp(f'{s0}=seam start', ra, pa, [('end0', 'bottom'), ('end1', 'top'), ('seam0', s0), ('seam1', s1)]))
    rep = {'scale': scale}
    ev = evaluate(ref, part, hyps, scale, ndense=65)
    e = _summarise(ev, rep, part, scale, issues)
    if e:
        ch = ref.chart
        ang, lam = ch.inverse(_to_ref(e, np.array([part.corner('seam_a', 'bottom'), part.corner('seam_a', 'top'), part.corner('seam_b', 'bottom'), part.corner('seam_b', 'top')])))
        P = ch.point3d(ang, lam)
        rep['seam_gap_3d'] = float(max(np.linalg.norm(P[0] - P[2]), np.linalg.norm(P[1] - P[3])))
        if rep['seam_gap_3d'] > rel_tol(scale):
            issues.append(f"cylinder: rolled seam edges do not meet in 3D (gap {rep['seam_gap_3d']:.3e})")
    return rep, ref


def _cmp_cone(ref, part, issues):
    scale = max(part.extent(), ref.info['scale'])
    c = ref.curves
    ra = [c['large'].f(0.0)[0], c['small'].f(0.0)[0], c['large'].f(TAU)[0], c['small'].f(TAU)[0]]
    hyps = []
    for s0, s1 in (('seam_a', 'seam_b'), ('seam_b', 'seam_a')):
        pa = [part.corner(s0, 'large_end'), part.corner(s0, 'small_end'), part.corner(s1, 'large_end'), part.corner(s1, 'small_end')]
        hyps.append(Hyp(f'{s0}=theta 0', ra, pa, [('large', 'large_end'), ('small', 'small_end'), ('seam0', s0), ('seam1', s1)]))
    rep = {'scale': scale, 'ref_apex_spread': ref.info['apex_spread'], 'ref_developability_defect': ref.info['developability_defect']}
    # numerically integrated family: the float reference resolves ~2e-9 of the size; the product's own stated
    # eps_num is honoured above that (and tested separately against mpmath), but never beyond 1e-6 of the size
    tol = max(rel_tol(scale), min(2 * part.eps_num, 1e-6 * scale))
    rep['tolerance'] = tol
    ev = evaluate(ref, part, hyps, scale, tol=tol)
    e = _summarise(ev, rep, part, scale, issues, tol=tol)
    if e:
        # conditioning of the fold-back: d(rho)/d(psi) along the large end
        amp = float(np.max(np.abs(np.gradient(np.linalg.norm(ref.c1.p(ref._t_tab) - ref.apex3, axis=1), ref._psi_tab)))) / scale
        tol = tol * max(1.0, amp) * 4
        rep['fold_conditioning'] = amp
        th, P, rho, rho3 = ref.fold(_to_ref(e, part.curve('large_end').vertices()))
        rep['fold_large_on_circle'] = float(np.abs(rho - rho3).max())
        th2, P2, rho_s, rho3_s = ref.fold(_to_ref(e, part.curve('small_end').vertices()))
        c2 = ref.c2
        d = P2 - c2.c
        rep['fold_small_on_circle'] = float(np.max(np.hypot(np.hypot(d[:, 0], d[:, 1]) - c2.r, d[:, 2])))
        corners = np.array([part.corner('seam_a', 'large_end'), part.corner('seam_b', 'large_end'), part.corner('seam_a', 'small_end'), part.corner('seam_b', 'small_end')])
        _, Pc, _, _ = ref.fold(_to_ref(e, corners))
        rep['seam_gap_3d'] = float(max(np.linalg.norm(Pc[0] - Pc[1]), np.linalg.norm(Pc[2] - Pc[3])))
        rep['turn_fraction'] = ref.info['psi_total'] / TAU
        rep['flat_seam_clearance'] = float(shapely.distance(shapely.LineString(part.curve('seam_a').pts), shapely.LineString(part.curve('seam_b').pts)))
        for k in ('fold_large_on_circle', 'fold_small_on_circle', 'seam_gap_3d'):
            if rep[k] > 5 * tol:
                issues.append(f'{part.name}: fold-back {k} = {rep[k]:.3e} exceeds {5 * tol:.3e}')
    return rep


def cmp_cone(req, part, issues):
    ref = RF.cone_frustum(req['r1'], req['r2'], req['h'])
    return _cmp_cone(ref, part, issues), ref


def cmp_eccentric(req, part, issues):
    ref = RF.eccentric_reducer(req['r1'], req['r2'], req['h'], req['e'])
    return _cmp_cone(ref, part, issues), ref


def cmp_branch(req, part, issues, ndense=6001):
    ref = RF.branch_stub(req['header_radius'], req['branch_radius'], req['branch_dev_radius'], req['beta_deg'], req['delta'], req['s_end'], req.get('branch_station', 0.0))
    scale = max(part.extent(), ref.info['scale'])
    c = ref.curves
    ra = [c['square_end'].f(0.0)[0], c['square_end'].f(TAU)[0], c['saddle'].f(0.0)[0], c['saddle'].f(TAU)[0]]
    hyps = []
    for s0, s1 in (('seam_a', 'seam_b'), ('seam_b', 'seam_a')):
        pa = [part.corner(s0, 'square_end'), part.corner(s1, 'square_end'), part.corner(s0, 'saddle_cut'), part.corner(s1, 'saddle_cut')]
        hyps.append(Hyp(f'{s0}=phi 0', ra, pa, [('square_end', 'square_end'), ('saddle', 'saddle_cut'), ('seam0', s0), ('seam1', s1)]))
    rep = {'scale': scale}
    tol = rel_tol(scale)
    onsurf = lambda pr: np.abs(ref.joint.dist_header_axis(RF.branch_fold(ref, pr)[1]) - ref.joint.R).max()
    ex = {'saddle_cut': _exact_pair(ref, 'saddle', tol, onsurf, 50 * tol / max(math.sin(math.radians(req['beta_deg'])), 1e-3))}
    ev = evaluate(ref, part, hyps, scale, ndense, exact=ex)
    e = _summarise(ev, rep, part, scale, issues)
    if e:
        rep['coverage_saddle'] = e['r2p']['saddle_cut']
        cov_tol = 1.10 * part.eps_rep + tol
        if rep['coverage_saddle'] > cov_tol:
            issues.append(f"{part.name}: the polyline does not cover the reference saddle: the reference curve lies {rep['coverage_saddle']:.3e} mm from the polyline, more than 1.10 eps_rep + tol = {cov_tol:.3e} mm")
        j = ref.joint
        phi, P = RF.branch_fold(ref, _to_ref(e, part.curve('saddle_cut').vertices()))
        rep['fold_cut_on_header'] = float(np.abs(j.dist_header_axis(P) - j.R).max())
        _, Pc = RF.branch_fold(ref, _to_ref(e, np.array([part.corner('seam_a', 'saddle_cut'), part.corner('seam_b', 'saddle_cut'), part.corner('seam_a', 'square_end'), part.corner('seam_b', 'square_end')])))
        rep['seam_gap_3d'] = float(max(np.linalg.norm(Pc[0] - Pc[1]), np.linalg.norm(Pc[2] - Pc[3])))
        if rep['fold_cut_on_header'] > 50 * rel_tol(scale) / max(math.sin(math.radians(req['beta_deg'])), 1e-3):
            issues.append(f"branch: folded saddle cut is off the header surface by {rep['fold_cut_on_header']:.3e}")
        if rep['seam_gap_3d'] > rel_tol(scale):
            issues.append(f"branch: rolled seam edges do not meet in 3D (gap {rep['seam_gap_3d']:.3e})")
    return rep, ref, ev


def cmp_header(req, part, issues, ndense=6001):
    ref = RF.header_with_opening(req['header_radius'], req['header_dev_radius'], req['header_length'], req['branch_station'], req['branch_radius'], req['beta_deg'], req['delta'])
    L = req['header_length']
    scale = max(part.extent(), ref.info['scale'])
    c = ref.curves
    ra = [c['seam0'].f(0.0)[0], c['seam0'].f(L)[0], c['seam1'].f(L)[0], c['seam1'].f(0.0)[0]]
    # the two short edges of the plate: which one carries station 0?  Natural reading: DXF x = station.
    shorts = sorted([n for n in ('top', 'bottom')], key=lambda n: part.curve(n).pts[:, 0].mean())
    hyps = []
    for e0, e1 in ((shorts[0], shorts[1]), (shorts[1], shorts[0])):
        for s0, s1 in (('seam_a', 'seam_b'), ('seam_b', 'seam_a')):
            pa = [part.corner(s0, e0), part.corner(s0, e1), part.corner(s1, e1), part.corner(s1, e0)]
            hyps.append(Hyp(f'station 0 at {e0} (x={part.curve(e0).pts[:, 0].mean():g}); {s0}=seam start', ra, pa,
                            [('end0', e0), ('end1', e1), ('seam0', s0), ('seam1', s1), ('opening', 'branch_opening')], natural=(e0 == shorts[0])))
    rep = {'scale': scale}
    tol = rel_tol(scale)
    onsurf = lambda pr: np.abs(ref.joint.dist_branch_axis(RF.header_fold(ref, pr)[2]) - ref.joint.r).max()
    ex = {'branch_opening': _exact_pair(ref, 'opening', tol, onsurf, 50 * tol / max(math.sin(math.radians(req['beta_deg'])), 1e-3))}
    ev = evaluate(ref, part, hyps, scale, ndense, exact=ex)
    e = _summarise(ev, rep, part, scale, issues)
    if e:
        rep['coverage_opening'] = e['r2p']['branch_opening']
        cov_tol = 1.10 * part.eps_rep + tol
        if rep['coverage_opening'] > cov_tol:
            issues.append(f"{part.name}: the polyline does not cover the reference opening: the reference curve lies {rep['coverage_opening']:.3e} mm from the polyline, more than 1.10 eps_rep + tol = {cov_tol:.3e} mm")
        j = ref.joint
        x, g, P = RF.header_fold(ref, _to_ref(e, part.curve('branch_opening').vertices()))
        rep['fold_opening_on_branch'] = float(np.abs(j.dist_branch_axis(P) - j.r).max())
        rep['opening_station_range'] = [float(x.min()), float(x.max())]
        rep['opening_seam_clearance'] = float(min(g.min(), TAU - g.max()) * req['header_dev_radius'])
        _, _, Pc = RF.header_fold(ref, _to_ref(e, np.array([part.corner('seam_a', 'top'), part.corner('seam_b', 'top'), part.corner('seam_a', 'bottom'), part.corner('seam_b', 'bottom')])))
        rep['seam_gap_3d'] = float(max(np.linalg.norm(Pc[0] - Pc[1]), np.linalg.norm(Pc[2] - Pc[3])))
        if rep['fold_opening_on_branch'] > 50 * rel_tol(scale) / max(math.sin(math.radians(req['beta_deg'])), 1e-3):
            issues.append(f"header: folded opening is off the branch surface by {rep['fold_opening_on_branch']:.3e}")
        if rep['seam_gap_3d'] > rel_tol(scale):
            issues.append(f"header: rolled seam edges do not meet in 3D (gap {rep['seam_gap_3d']:.3e})")
    return rep, ref, ev


def _dist_pts_polyline3d(P, Q):
    """Exact distance from each 3D point P[i] to the 3D polyline Q (minimum over all segments).  The candidates for P[i]
    are the segments whose midpoint lies within (distance to the nearest vertex + half the longest segment): every
    segment that could be the nearest one is among them, so this equals a search over all segments, only faster."""
    from scipy.spatial import cKDTree
    P, Q = np.asarray(P, float), np.asarray(Q, float)
    a, b = Q[:-1], Q[1:]
    ab = b - a
    L2 = np.maximum((ab * ab).sum(1), 1e-300)
    half = 0.5 * float(np.sqrt(L2).max())
    dv, _ = cKDTree(Q).query(P)
    cand = cKDTree(0.5 * (a + b)).query_ball_point(P, dv + half)
    out = np.empty(len(P))
    for i, idx in enumerate(cand):
        idx = np.asarray(idx, dtype=int)
        p = P[i]
        u = np.clip(((p - a[idx]) * ab[idx]).sum(1) / L2[idx], 0.0, 1.0)
        out[i] = np.linalg.norm(p - (a[idx] + u[:, None] * ab[idx]), axis=1).min()
    return out


def mating(req, hpart, bpart, href, bref, hev, bev):
    """3D coincidence of header opening and branch saddle for the four combinations of drawing side."""
    res = {}
    for hv in ('outside', 'inside'):
        for bv in ('outside', 'inside'):
            he = next((e for e in hev if e['natural'] and e['view'] == hv and e['anchor_residual'] <= 1e-6 * href.info['scale']), None)
            be = next((e for e in bev if e['view'] == bv and e['anchor_residual'] <= 1e-6 * bref.info['scale']), None)
            if he is None or be is None:
                res[f'header {hv} / branch {bv}'] = None
                continue
            _, _, PH = RF.header_fold(href, _to_ref(he, hpart.curve('branch_opening').vertices()))
            _, PB = RF.branch_fold(bref, _to_ref(be, bpart.curve('saddle_cut').vertices()))
            d = max(_dist_pts_polyline3d(PH, PB).max(), _dist_pts_polyline3d(PB, PH).max())
            res[f'header {hv} / branch {bv}'] = float(d)
    return res


def declared_side(part, pr, issues, findings, chiral):
    """The result and the DXF must declare a marking side, the same one, and the geometry must really be drawn from it.
    A withheld part has no file to read the marking text from: that half of the check is recorded not-applicable
    (never silently dropped); the result-side declaration and the drawn-side test still run on the JSON geometry."""
    side = part.view.get('marking_side')
    pr['marking_side_declared'] = side
    if part.from_file:
        texts = [getattr(e, 'text', '').upper() for e in part.other if e.kind == 'text']
        dxf_side = next(('outside' if 'OUTSIDE' in t else 'inside' if 'INSIDE' in t else None for t in texts if 'MARKING SIDE' in t), None)
        pr['marking_side_in_dxf'] = dxf_side
    else:
        dxf_side = None
        pr['marking_side_in_dxf'] = None
        pr.setdefault('not_applicable', []).append('marking side text in the DXF (file withheld)')
    if side not in ('outside', 'inside'):
        findings.append({'code': 'MARKING_SIDE_UNDECLARED', 'detail': f'{part.name}: the result declares no marking side (Spec 02 0.3)'})
        return
    if part.from_file:
        if dxf_side is None:
            findings.append({'code': 'MARKING_SIDE_UNDECLARED', 'detail': f'{part.name}: the DXF file does not state the marking side'})
        elif dxf_side != side:
            issues.append(f'{part.name}: DXF states marking side {dxf_side}, result states {side}')
    views = pr.get('views_matching')
    if views is not None and side not in views:
        issues.append(f'{part.name}: declared as drawn from the {side.upper()}, but the geometry matches the reference only as seen from the {"/".join(views) or "?"}')


def is_chiral(req, threshold=1e-9):
    """A branch joint is mirror-asymmetric iff it is both offset and oblique."""
    R = req['header_radius']
    return abs(req['delta']) > threshold * R and abs(math.cos(math.radians(req['beta_deg']))) > threshold



SEAM_RESOLUTION = 1e-9      # mm, closure resolution of the profile check (the same constant as checks/oracle.py)
CHORD_LIMIT = 0.01          # mm, the chord-error limit every polyline meets (TOLERANCE_POLICY section 5)
HALF_ULP = 2.0 ** -53       # binary64 unit roundoff, used only for the independent lower bound on the numerical error of a written vertex


def seam_resolution_check(fam, name, pr, job, issues):
    """Engine 0.4.2, Q2(b). Rule from the specification of the profile check, not from the engine: two seam edges whose clearance is
    smaller than the error of the curves that end on them cannot be told from a closed seam (nor drawn apart in a DXF), so the profile
    gate (G11) must not say PASS. The floor of that error is stated per family:
      right cone: its arcs are exact entities and its seam edges straight lines, but every written vertex carries the numerical error
        eps_num of the product (reported, an a-priori bound that grows with the size of the part): floor = max(1e-9 mm, 2 eps_num). The
        floor is NOT the bare 1e-9 mm closure resolution: at an apex radius of 1e7 mm, 2 eps_num is about 1.5e-7 mm, and a seam gap
        between the two cannot be represented in the DXF (G15 then rejects the job).
      eccentric reducer: its large and small ends are polylines, so twice the true representation error measured here against the
        reference curve (never above twice the 0.01 mm chord limit), and twice eps_num.
    The reported eps_num is not taken on trust: it must be at least the rounding of the largest coordinate the DXF carries (extent x 2^-53,
    computed here from the DXF), otherwise the product understates its own error and the check says so. The clearance is MEASURED here from
    the product's own seam edges (reported `seam_gap` is the engine's chord between the seam end points; the measured clearance is the
    distance between the two seam edges; they must agree within 1e-3 relative plus 8 eps_num).
    Changed validation rule (0.4.2 review must-fix 2): the cone floor was the constant 1e-9 mm; it is now max(1e-9, 2 eps_num) because
    the constant was wrong where 2 eps_num exceeds it (large cones)."""
    gap = pr['flat_seam_clearance']
    pv = next((p for p in job.result.get('patterns', []) if p['part'] == name), {})
    q = pv.get('quantities', {})
    eps_num = pv.get('eps_num', 0.0)
    part = job.parts[name]
    # extent of the written coordinates: from the DXF entities, or — for a withheld part — from the JSON vertices,
    # which are the same drawn coordinates (a withheld part carries no arc, whose exact geometry the JSON omits),
    # so the eps_num floor below is unchanged and the check still runs
    eps_floor = HALF_ULP * part.extent()
    if eps_num < eps_floor:
        issues.append(f'{name}: reported eps_num {eps_num:.3g} mm is below the rounding of the largest written coordinate ({eps_floor:.3g} mm)')
    chord = pr.get('chord_dev', 0.0) if fam == 'eccentric_reducer' else 0.0
    thr = max(SEAM_RESOLUTION, 2.0 * chord, 2.0 * eps_num)
    cap = max(SEAM_RESOLUTION, 2.0 * CHORD_LIMIT, 2.0 * eps_num) if fam == 'eccentric_reducer' else thr
    g11 = next((g['result'] for g in job.result.get('gates', []) if g['id'] == 'G11'), None)
    tol = 8 * eps_num + 1e-12
    if 'seam_gap' not in q or 'seam_gap_resolution' not in q:
        issues.append(f'{name}: the pattern does not report seam_gap / seam_gap_resolution')
    else:
        if abs(q['seam_gap'] - gap) > 1e-3 * gap + tol:
            issues.append(f'{name}: reported seam_gap {q["seam_gap"]:.6g} mm differs from the measured seam clearance {gap:.6g} mm')
        want = max(SEAM_RESOLUTION, 2.0 * pv.get('eps_rep', 0.0), 2.0 * eps_num) if fam == 'eccentric_reducer' else max(SEAM_RESOLUTION, 2.0 * eps_num)
        if abs(q['seam_gap_resolution'] - want) > 1e-12 * want:
            issues.append(f'{name}: seam_gap_resolution {q["seam_gap_resolution"]:.6g} mm is not the specified {want:.6g} mm')
    if gap < thr * (1 - 1e-3) and g11 == 'PASS':
        issues.append(f'{name}: seam clearance {gap:.4g} mm is below the resolution {thr:.4g} mm of its adjoining curves, but the profile gate G11 says PASS')
    if gap > cap * (1 + 1e-3) and g11 != 'PASS':
        issues.append(f'{name}: seam clearance {gap:.4g} mm is above any resolution ({cap:.4g} mm) but the profile gate G11 says {g11}')


def compare_job(req, job, ndense=6001, dxf=True):
    """Full comparison.  Returns report dict with 'issues' (geometry disagreements) and 'findings' (other defects).
    dxf=False skips the DXF round trip (used only by the seeded-fault tests, which plant geometry, not files)."""
    issues, findings, rep = [], [], {'parts': {}}
    fam = req['family']
    for name, why in job.unbuilt.items():
        # a part the job could not build from the result is never compared, and never passes silently
        issues.append(f'{name}: cannot be compared against the reference: {why}')
    for name, part in job.parts.items():
        if dxf and part.from_file:
            pr = {'dxf': dxf_roundtrip(part, issues)}
        elif dxf:
            # the file is withheld (INCONCLUSIVE checks on the cut geometry): the round trip against the file and the
            # other file-level checks cannot run.  The geometry-only checks still run on the JSON points (json_geometry),
            # and the geometry comparison runs below on the part built from the JSON curves.
            pr = {'dxf': json_geometry(part, issues),
                  'not_applicable': ['DXF round trip against the file (JSON points vs DXF vertices, entity kinds and layers, entity list, units / header)',
                                     'served DXF hash against dxf_sha256',
                                     'extents header ($EXTMIN/$EXTMAX) against the true extents',
                                     'ezdxf and the minimal reader agreement']}
        else:
            pr = {'dxf': {}}
        rep['parts'][name] = pr
    # A withheld file must be withheld ONLY because checks on that part's cut geometry are INCONCLUSIVE: the result
    # must list the part in dxf_withheld, every listed check must be INCONCLUSIVE in the part's or the interface
    # scope, and the 409 must name exactly the listed checks.  Anything else is a defect (a file held back for a
    # wrong reason, or a mismatch between the result and the served refusal).
    withhold_ok = {}
    listed_map = (job.result.get('dxf_withheld') or {}) if job.result else {}
    for name in job.withheld:
        listed = set(listed_map.get(name, []))
        scoped = [c for c in job.checks() if c.get('scope') in (name, 'interface')]
        unsure = {c['name'] for c in scoped if c.get('verdict') == 'INCONCLUSIVE'}
        served = {c.get('name') for c in job.withheld[name].checks}
        reasons = []
        if name not in listed_map:
            reasons.append('the result does not list this part in dxf_withheld')
        elif not listed:
            reasons.append('the result lists this part in dxf_withheld with no check: a file held back for no stated reason')
        if not listed <= unsure:
            reasons.append(f'listed checks not INCONCLUSIVE in the part/interface scope: {sorted(listed - unsure)}')
        if served != listed:
            reasons.append(f'the 409 names {sorted(served)} but the result lists {sorted(listed)}')
        if served != unsure:
            reasons.append(f'the 409 names {sorted(served)} but the INCONCLUSIVE checks in the part/interface scope are {sorted(unsure)}')
        withhold_ok[name] = not reasons
        rep.setdefault('withheld_checks', {})[name] = {
            'name': 'dxf withheld only because of INCONCLUSIVE checks',
            'result': 'PASS' if not reasons else 'FAIL',
            'listed': sorted(listed), 'served_409': sorted(served),
            **({'reasons': reasons} if reasons else {})}
        if reasons:
            issues.append(f'{name}: dxf withheld only because of INCONCLUSIVE checks: FAIL ({"; ".join(reasons)})')
    for name in listed_map:
        # the reverse of the rule above: a part the result lists as withheld must actually have been withheld (409)
        if name not in job.withheld:
            issues.append(f'{name}: the result lists this part in dxf_withheld, but its DXF was not withheld (DXF status {job.dxf_status.get(name)})')
    for name, st in job.dxf_status.items():
        # a 409 is excused only for a part whose withhold check above PASSES; every other non-200 stays an issue
        if st != 200 and not withhold_ok.get(name):
            issues.append(f'{name}: DXF endpoint returned {st} for an accepted job')
        # the reverse rule: a part with ANY INCONCLUSIVE check in its own or the interface scope must be withheld;
        # a served DXF (200, part not withheld) means the engine broke the withhold rule
        if st == 200 and name not in job.withheld:
            unsure = sorted({c['name'] for c in job.checks() if c.get('scope') in (name, 'interface') and c.get('verdict') == 'INCONCLUSIVE'})
            if unsure:
                issues.append(f'{name}: DXF served although checks {unsure} are INCONCLUSIVE')
    try:
        if fam == 'cylinder_shell':
            r, _ = cmp_cylinder(req, job.parts['shell'], issues)
            rep['parts']['shell'].update(r)
        elif fam == 'cone_frustum':
            r, _ = cmp_cone(req, job.parts['reducer'], issues)
            rep['parts']['reducer'].update(r)
        elif fam == 'eccentric_reducer':
            r, _ = cmp_eccentric(req, job.parts['reducer'], issues)
            rep['parts']['reducer'].update(r)
        elif fam == 'branch_stub':
            r, _, ev = cmp_branch(req, job.parts['branch'], issues, ndense)
            rep['parts']['branch'].update(r)
            rep['chiral'] = is_chiral(req)
        elif fam == 'header_with_branch':
            hr, href, hev = cmp_header(req, job.parts['header'], issues, ndense)
            br, bref, bev = cmp_branch(req, job.parts['branch'], issues, ndense)
            rep['parts']['header'].update(hr)
            rep['parts']['branch'].update(br)
            rep['chiral'] = is_chiral(req)
            mt = mating(req, job.parts['header'], job.parts['branch'], href, bref, hev, bev)
            rep['mating_3d'] = mt
            hpart, bpart = job.parts['header'], job.parts['branch']
            tol = hpart.eps_rep + bpart.eps_rep + rel_tol(max(hr['scale'], br['scale'])) * 50
            best = min((v for v in mt.values() if v is not None), default=None)
            rep['mating_best'] = best
            rep['mating_tol'] = tol
            if best is None or best > 3 * tol:
                issues.append(f'assembly: header opening and branch saddle do not coincide in 3D under any drawing-side combination (best {best})')
            same = [mt.get('header outside / branch outside'), mt.get('header inside / branch inside')]
            mixed = [mt.get('header outside / branch inside'), mt.get('header inside / branch outside')]
            ok = lambda v: v is not None and v <= 3 * tol
            rep['mating_same_side_ok'] = any(ok(v) for v in same)
            rep['mating_mixed_side_ok'] = any(ok(v) for v in mixed)
            if not rep['mating_same_side_ok'] and rep['mating_mixed_side_ok']:
                worst = min(v for v in same if v is not None)
                findings.append({'code': 'VIEW_SIDE_CONFLICT', 'detail': f'header and branch patterns are drawn from opposite sides of the plate; marked on the same side the saddle misses the opening by {worst:.4g} mm in 3D'})
                rep['same_side_misfit_mm'] = worst
        else:
            issues.append(f'unknown family {fam}')
    except Exception as ex:  # a comparison that cannot be completed is a failure, never a pass
        import traceback
        issues.append(f'comparison aborted: {type(ex).__name__}: {ex} | {traceback.format_exc(limit=2)}')
    for name, pr in rep['parts'].items():
        part = job.parts[name]
        if 'chord_dev' in pr:
            tol = rel_tol(pr['scale'])
            # eps_rep is declared an estimate (not a bound); an estimate that is low by more than 10 % is reported
            if pr['chord_dev'] > part.eps_rep * 1.10 + tol:
                findings.append({'code': 'EPS_REP_EXCEEDED', 'detail': f"{name}: true curve-representation error {pr['chord_dev']:.4g} mm exceeds stated eps_rep {part.eps_rep:.4g} mm (x{pr['chord_dev'] / part.eps_rep if part.eps_rep else float('inf'):.2f})"})
        d = pr['dxf']
        if pr.get('turn_fraction', 0.0) >= 0.999 and pr.get('flat_seam_clearance', 1.0) <= 1e-9:     # engine 0.4.2 (B2-005): a cone sector never overlaps; its seam edges are a defect only when they cannot be told apart, i.e. at or below the 1e-9 mm closure resolution (was: 99.9 % of a turn)
            findings.append({'code': 'FLAT_SEAM_CLEARANCE', 'detail': f"{name}: the two seam edges of the flat pattern are {pr['flat_seam_clearance']:.3g} mm apart (pattern closes on itself)"})
        if 'flat_seam_clearance' in pr:
            seam_resolution_check(fam, name, pr, job, issues)
        if d.get('extents_header_error', 0) > 1e-6 * max(pr.get('scale', 1), 1e-300):
            findings.append({'code': 'DXF_EXTENTS_WRONG', 'detail': f"{name}: $EXTMIN/$EXTMAX differ from the true extents by {d['extents_header_error']:.4g} mm"})
        if d.get('json_arc_sagitta', 0) > 1e-9 * max(pr.get('scale', 1), 1e-300) and not d.get('json_arc_labelled', False):
            findings.append({'code': 'JSON_ARC_UNLABELLED', 'detail': f"{name}: JSON 'points' of an arc deviate {d['json_arc_sagitta']:.4g} mm from the arc while eps_rep = {part.eps_rep}"})
    # declared marking side against the side the geometry is actually drawn from (reference = outside view by construction)
    sides = {}
    for name, pr in rep['parts'].items():
        declared_side(job.parts[name], pr, issues, findings, rep.get('chiral', False))
        sides[name] = pr.get('marking_side_declared')
    # the file-level checks skipped for a withheld part (round trip, hash, extents header, reader agreement, and the
    # marking-side text), collected per part so the report shows exactly what was not applicable and why
    na = {name: pr['not_applicable'] for name, pr in rep['parts'].items() if pr.get('not_applicable')}
    if na:
        rep['not_applicable'] = na
    if len(set(sides.values())) > 1:
        issues.append(f'parts of one job declare different marking sides: {sides}')
    if fam == 'header_with_branch' and 'mating_3d' in rep and len(set(sides.values())) == 1 and list(sides.values())[0] in ('outside', 'inside'):
        s = list(sides.values())[0]
        v = rep['mating_3d'].get(f'header {s} / branch {s}')
        rep['mating_on_declared_side'] = v
        if v is None or v > 3 * rep['mating_tol']:
            issues.append(f'assembly: folded from the declared marking side ({s}) the saddle misses the opening by {v} mm')
    # The product's own verdicts (task 1.2 review; plan rule A9). Derivation, from the specification:
    # TOLERANCE_POLICY section 8 defines three verdicts. PASS: the check holds to the stated precision. FAIL: the check
    # is violated, so the pattern is wrong (a defect: PRODUCT_CHECK_NOT_PASS, in DEFECT_CODES). INCONCLUSIVE: the
    # check cannot be certified to the required precision because a bound is an estimate or too coarse (the product
    # fails closed; the result is not shown to be wrong), so it is an observation, PRODUCT_CHECK_INCONCLUSIVE, that
    # is reported in findings_by_code but is NOT in DEFECT_CODES (validation/run_all.py). Any verdict outside the
    # three is not a known outcome and stays a defect. This reference does not rely on the product's verdicts to
    # detect a fault: a planted fault is caught by the independent comparison through `issues` (cmp_header, cmp_branch,
    # declared_side). Findings never feed `agree` (rep['agree'] = not issues), so the split changes no disagree or
    # wrongly_accepted outcome. Seeded-fault test: validation/seeded_faults.py, verdict_faults().
    verdicts = [(c['name'], c['verdict']) for c in job.checks()]
    bad = [n for n, v in verdicts if v not in ('PASS', 'INCONCLUSIVE')]
    unsure = [n for n, v in verdicts if v == 'INCONCLUSIVE']
    if bad:
        findings.append({'code': 'PRODUCT_CHECK_NOT_PASS', 'detail': f'product verification checks FAIL (or are not a known verdict): {bad}'})
    if unsure:
        findings.append({'code': 'PRODUCT_CHECK_INCONCLUSIVE', 'detail': f'product verification checks INCONCLUSIVE (not certified to the required precision, not a wrong result): {unsure}'})
    rep['issues'], rep['findings'] = issues, findings
    rep['agree'] = not issues
    return rep


def compare_scaled(job, j2, k, other, dxf=True):
    """Scale relation: the pattern of the request scaled by k must be k times the pattern of the base request.
    (1) Entity types and counts agree.  Vertex counts may differ: the product's polyline density follows an absolute
        chord-error limit, so it changes with size.
    (2) Every polyline agrees with k times the base polyline: two-sided vertex-to-segment distance d <= tau, with
        tau = k * eps_rep(base) + eps_rep(scaled) + 1e-11 * extent(scaled): the stated chord-error limits of the two
        patterns plus the numerical tolerance of the relation.  Lines and arcs carry no chord error: d <= 1e-11 * extent.
    (3) The scaled pattern must pass the exact reference comparison (compare_job).  Step (2) alone cannot see a deviation
        below the chord-error budget (for example a relative 1e-6 fault, far below k * eps_rep for these patterns).
    Returns (type_issues, deviations, findings, info); the caller decides whether deviations are issues or defects."""
    type_issues, devs, info, worst = [], [], {}, 0.0
    missing = info['scale_missing_parts'] = []
    for name, p1 in job.parts.items():
        if name not in j2.parts:
            # the scaled job did not build this part (its DXF was neither served nor withheld for INCONCLUSIVE
            # checks, or the part cannot be built from the JSON): record it visibly, never silently skip.  It is an
            # issue of its own, listed in info['scale_missing_parts'], and is never counted as a detected scale fault.
            msg = f'scale x{k}: {name} has no part in the scaled job (DXF status {j2.dxf_status.get(name)}; {j2.unbuilt.get(name, "no part built")})'
            type_issues.append(msg)
            missing.append(msg)
            continue
        p2 = j2.parts[name]
        sc2 = max(p2.extent(), 1e-300)
        num = 1e-11 * sc2
        if len(p1.ents) != len(p2.ents):
            type_issues.append(f'scale x{k}: {name} has {len(p2.ents)} cut entities, the base has {len(p1.ents)}')
            continue
        for e1, e2 in zip(p1.ents, p2.ents):
            if e1.kind != e2.kind:
                type_issues.append(f'scale x{k}: {name}/{e1.name} changes entity type ({e1.kind} -> {e2.kind})')
                continue
            if e1.kind == 'arc':
                dev = max(float(np.abs(e2.pts - k * e1.pts).max()), abs(e2.r - k * e1.r),
                          sc2 * max(abs(e2.a0 - e1.a0), abs(e2.a1 - e1.a1)) * math.pi / 180)
                tau = num
            else:
                dev = two_sided(k * e1.pts, e2.pts)
                tau = (k * p1.eps_rep + p2.eps_rep if e1.kind == 'polyline' else 0.0) + num
            worst = max(worst, dev / tau)
            if dev > tau:
                devs.append(f'scale x{k}: {name}/{e1.name} ({e1.kind}) lies {dev:.3e} mm from k times the base entity, allowed {tau:.3e} mm')
    info['scale_worst_dev_over_tau'] = worst
    rep = compare_job(other, j2, dxf=dxf)
    info['scale_exact_issues'] = len(rep['issues'])
    devs += [f'scaled pattern fails the exact reference comparison: {s}' for s in rep['issues']]
    return type_issues, devs, list(rep['findings']), info
