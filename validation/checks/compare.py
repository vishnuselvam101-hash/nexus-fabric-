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


# ------------------------------------------------------------------ 1. DXF round trip
def dxf_roundtrip(part, issues):
    v = part.view
    scale = max(part.extent(), 1e-300)
    tol = 1e-12 * scale
    jc = v['curves'] + v['holes']
    m = {'entities': len(part.ents)}
    if hashlib.sha256(part.dxf_text.encode()).hexdigest() != v.get('dxf_sha256'):
        issues.append(f'{part.name}: served DXF does not hash to dxf_sha256 of the result')
    if len(part.ents) != len(jc):
        issues.append(f'{part.name}: DXF has {len(part.ents)} entities, JSON has {len(jc)} curves')
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
            # how far the JSON display polyline is from the true arc
            mid = 0.5 * (jp[:-1] + jp[1:])
            m['json_arc_sagitta'] = max(m['json_arc_sagitta'], float(e.dist(mid).max()))
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
    # header data of the file
    mm = part.meta_min
    m['units_declared'] = 'comment-only' if any('millimetre' in c.lower() for c in mm['comment']) else 'none'
    m['insunits'] = mm['insunits']
    if not mm['eof']:
        issues.append(f'{part.name}: DXF does not end with EOF')
    bb = part.measures()
    if 'extmin' in mm and 'extmax' in mm:
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


def evaluate(ref, part, hyps, scale, ndense=6001, tol=None):
    """Try every labelling hypothesis with and without mirroring.  Returns list of dicts."""
    tol = rel_tol(scale) if tol is None else tol
    out = []
    for h in hyps:
        for proper in (True, False):
            R, t, res = fit_rigid(h.ref_pts, h.prod_pts, proper)
            e = {'label': h.label, 'natural': h.natural, 'view': 'outside' if proper else 'inside', 'anchor_residual': res, 'R': R, 't': t, 'r2p': {}, 'p2r': {}}
            if res <= 1e-6 * scale:
                for rn, pn in h.pairs:
                    rc, pc = ref.curves[rn], part.curve(pn)
                    f = lambda tt, rc=rc: rc.f(tt) @ R.T + t
                    pts = f(np.linspace(rc.t0, rc.t1, ndense))
                    ok = np.all(np.isfinite(pts), axis=1)
                    e['r2p'][pn] = float(pc.dist(pts[ok]).max()) if ok.any() else float('nan')
                    d, _ = dist_pts_to_param_curve(f, rc.t0, rc.t1, pc.vertices())
                    e['p2r'][pn] = float(np.nanmax(d))
                e['vertex_dev'] = max(e['p2r'].values())
                e['chord_dev'] = max(e['r2p'].values())
                e['match'] = bool(res <= tol and e['vertex_dev'] <= tol)
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
    ev = evaluate(ref, part, hyps, scale, ndense)
    e = _summarise(ev, rep, part, scale, issues)
    if e:
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
    ev = evaluate(ref, part, hyps, scale, ndense)
    e = _summarise(ev, rep, part, scale, issues)
    if e:
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
    """Distance from each 3D point P[i] to the 3D polyline Q."""
    a, b = Q[:-1], Q[1:]
    ab = b - a
    L2 = np.maximum((ab * ab).sum(1), 1e-300)
    out = np.empty(len(P))
    for i0 in range(0, len(P), 256):
        p = P[i0:i0 + 256, None, :]
        u = np.clip(((p - a) * ab).sum(-1) / L2, 0, 1)
        d = np.linalg.norm(p - (a + u[..., None] * ab), axis=-1)
        out[i0:i0 + 256] = d.min(1)
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


def is_chiral(req, threshold=1e-9):
    """A branch joint is mirror-asymmetric iff it is both offset and oblique."""
    R = req['header_radius']
    return abs(req['delta']) > threshold * R and abs(math.cos(math.radians(req['beta_deg']))) > threshold


def compare_job(req, job, ndense=6001):
    """Full comparison.  Returns report dict with 'issues' (geometry disagreements) and 'findings' (other defects)."""
    issues, findings, rep = [], [], {'parts': {}}
    fam = req['family']
    for name, part in job.parts.items():
        pr = {'dxf': dxf_roundtrip(part, issues)}
        rep['parts'][name] = pr
    for name, st in job.dxf_status.items():
        if st != 200:
            issues.append(f'{name}: DXF endpoint returned {st} for an accepted job')
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
            if pr['chord_dev'] > part.eps_rep * 1.02 + tol:
                findings.append({'code': 'EPS_REP_EXCEEDED', 'detail': f"{name}: true curve-representation error {pr['chord_dev']:.4g} mm exceeds stated eps_rep {part.eps_rep:.4g} mm (x{pr['chord_dev'] / part.eps_rep if part.eps_rep else float('inf'):.2f})"})
        d = pr['dxf']
        if pr.get('flat_seam_clearance', 1.0) < 1e-3:
            findings.append({'code': 'FLAT_SEAM_CLEARANCE', 'detail': f"{name}: the two seam edges of the flat pattern are {pr['flat_seam_clearance']:.3g} mm apart (pattern closes on itself)"})
        if d.get('extents_header_error', 0) > 1e-6 * max(pr.get('scale', 1), 1e-300):
            findings.append({'code': 'DXF_EXTENTS_WRONG', 'detail': f"{name}: $EXTMIN/$EXTMAX differ from the true extents by {d['extents_header_error']:.4g} mm"})
        if d.get('json_arc_sagitta', 0) > 1e-3:
            findings.append({'code': 'JSON_ARC_COARSE', 'detail': f"{name}: JSON 'points' of an arc deviate {d['json_arc_sagitta']:.4g} mm from the arc while eps_rep = {part.eps_rep}"})
        # the product's own checks must all have passed for an accepted job
    bad = [c['name'] for c in job.checks() if c['verdict'] != 'PASS']
    if bad:
        findings.append({'code': 'PRODUCT_CHECK_NOT_PASS', 'detail': f'product verification checks not PASS: {bad}'})
    rep['issues'], rep['findings'] = issues, findings
    rep['agree'] = not issues
    return rep
