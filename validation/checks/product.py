"""Product result model: the job JSON plus the DXF files it serves, read with
two independent DXF readers (ezdxf and a minimal group-code parser written
here).  The DXF is treated as the authoritative geometry (it is what would be
cut); the JSON is compared against it (round trip)."""
import io
import math
import numpy as np
import shapely

try:
    import ezdxf
    HAVE_EZDXF = True
except Exception:  # pragma: no cover
    HAVE_EZDXF = False


# ------------------------------------------------------------------ entities
class PCurve:
    """One product curve.  kind: line | arc | polyline."""

    def __init__(self, kind, layer, pts=None, c=None, r=None, a0=None, a1=None, name=None):
        self.kind, self.layer, self.name = kind, layer, name
        self.c, self.r, self.a0, self.a1 = c, r, a0, a1      # arc: counter-clockwise from a0 to a1 (degrees), DXF convention
        if kind == 'arc':
            self.sweep = math.radians((a1 - a0) % 360.0) if (a1 - a0) % 360.0 != 0 else (2 * math.pi if a1 != a0 else 0.0)
            self.pts = self.arc_points(2)
        else:
            self.pts = np.asarray(pts, float)
        if kind != 'arc':
            self._ls = shapely.LineString(self.pts) if len(self.pts) > 1 else None

    def arc_points(self, n):
        a = math.radians(self.a0) + np.linspace(0.0, self.sweep, n)
        return np.stack([self.c[0] + self.r * np.cos(a), self.c[1] + self.r * np.sin(a)], axis=1)

    @property
    def start(self):
        return self.pts[0]

    @property
    def end(self):
        return self.pts[-1]

    def vertices(self, n_arc=97):
        return self.arc_points(n_arc) if self.kind == 'arc' else self.pts

    def dense(self, n_arc=2049):
        return self.arc_points(n_arc) if self.kind == 'arc' else self.pts

    def dist(self, pts):
        """Distance from points to the exact entity geometry."""
        pts = np.asarray(pts, float)
        if self.kind != 'arc':
            return shapely.distance(shapely.points(pts), self._ls)
        v = pts - np.asarray(self.c)
        rad = np.hypot(v[:, 0], v[:, 1])
        ang = (np.arctan2(v[:, 1], v[:, 0]) - math.radians(self.a0)) % (2 * math.pi)
        inside = ang <= self.sweep
        d_end = np.minimum(np.linalg.norm(pts - self.pts[0], axis=1), np.linalg.norm(pts - self.pts[-1], axis=1))
        return np.where(inside, np.abs(rad - self.r), d_end)

    def length(self):
        if self.kind == 'arc':
            return self.r * self.sweep
        return float(np.linalg.norm(np.diff(self.pts, axis=0), axis=1).sum())

    def area_term(self, reverse=False):
        """Contribution to (1/2) closed-integral (x dy - y dx), traversed start->end (or reversed)."""
        if self.kind == 'arc':
            (x0, y0), (x1, y1) = self.pts[0], self.pts[-1]
            v = 0.5 * (self.r ** 2 * self.sweep + self.c[0] * (y1 - y0) - self.c[1] * (x1 - x0))
        else:
            p = self.pts
            v = 0.5 * float(np.sum(p[:-1, 0] * p[1:, 1] - p[1:, 0] * p[:-1, 1]))
        return -v if reverse else v

    def bbox(self):
        pts = self.pts
        if self.kind == 'arc':
            ext = [self.pts[0], self.pts[-1]]
            for k in range(-4, 9):
                q = k * 90.0
                if ((q - self.a0) % 360.0) <= math.degrees(self.sweep):
                    ext.append([self.c[0] + self.r * math.cos(math.radians(q)), self.c[1] + self.r * math.sin(math.radians(q))])
            pts = np.array(ext)
        return pts[:, 0].min(), pts[:, 1].min(), pts[:, 0].max(), pts[:, 1].max()


def read_dxf_ezdxf(text):
    doc = ezdxf.read(io.StringIO(text))
    ents = []
    for e in doc.modelspace():
        t = e.dxftype()
        if t == 'LINE':
            ents.append(PCurve('line', e.dxf.layer, [[e.dxf.start.x, e.dxf.start.y], [e.dxf.end.x, e.dxf.end.y]]))
        elif t == 'ARC':
            ents.append(PCurve('arc', e.dxf.layer, c=(e.dxf.center.x, e.dxf.center.y), r=e.dxf.radius, a0=e.dxf.start_angle, a1=e.dxf.end_angle))
        elif t == 'POLYLINE':
            pts = [[p.x, p.y] for p in e.points()]
            if e.is_closed:
                pts.append(pts[0])
            ents.append(PCurve('polyline', e.dxf.layer, pts))
        elif t == 'TEXT':
            ents.append(PCurve('text', e.dxf.layer, [[e.dxf.insert.x, e.dxf.insert.y], [e.dxf.insert.x, e.dxf.insert.y]]))
            ents[-1].text = e.dxf.text
        else:
            ents.append(PCurve('unknown:' + t, e.dxf.layer, [[0, 0], [0, 0]]))
    meta = {'version': doc.dxfversion, 'insunits': doc.header.get('$INSUNITS', None),
            'extmin': tuple(doc.header.get('$EXTMIN', (None, None)))[:2], 'extmax': tuple(doc.header.get('$EXTMAX', (None, None)))[:2],
            'layers': sorted(l.dxf.name for l in doc.layers)}
    return ents, meta


def read_dxf_minimal(text):
    """Independent minimal reader: group-code pairs -> LINE / ARC / POLYLINE(VERTEX..SEQEND)."""
    lines = text.splitlines()
    if len(lines) % 2:
        raise ValueError('odd number of lines')
    pairs = [(int(lines[i].strip()), lines[i + 1].strip()) for i in range(0, len(lines), 2)]
    meta = {'comment': [v for c, v in pairs if c == 999], 'eof': pairs[-1] == (0, 'EOF'), 'acadver': None, 'insunits': None, 'measurement': None}
    for i, (c, v) in enumerate(pairs):
        if c == 9 and v == '$ACADVER':
            meta['acadver'] = pairs[i + 1][1]
        if c == 9 and v == '$INSUNITS':
            meta['insunits'] = pairs[i + 1][1]
        if c == 9 and v in ('$LUNITS', '$AUNITS', '$ANGDIR', '$ANGBASE'):
            meta[v[1:].lower()] = pairs[i + 1][1]
        if c == 9 and v == '$MEASUREMENT':
            meta['measurement'] = pairs[i + 1][1]
        if c == 9 and v in ('$EXTMIN', '$EXTMAX'):
            meta[v[1:].lower()] = (float(pairs[i + 1][1]), float(pairs[i + 2][1]))
    start = next(i for i, p in enumerate(pairs) if p == (2, 'ENTITIES'))
    ents, i, poly = [], start + 1, None
    while i < len(pairs):
        c, v = pairs[i]
        j = i + 1
        while j < len(pairs) and pairs[j][0] != 0:
            j += 1
        body = dict()
        for cc, vv in pairs[i + 1:j]:
            body.setdefault(cc, vv)
        if c == 0:
            if v == 'LINE':
                ents.append(PCurve('line', body.get(8, '0'), [[float(body[10]), float(body[20])], [float(body[11]), float(body[21])]]))
            elif v == 'ARC':
                ents.append(PCurve('arc', body.get(8, '0'), c=(float(body[10]), float(body[20])), r=float(body[40]), a0=float(body[50]), a1=float(body[51])))
            elif v == 'POLYLINE':
                poly = {'layer': body.get(8, '0'), 'closed': int(body.get(70, '0')) & 1, 'pts': []}
            elif v == 'VERTEX' and poly is not None:
                poly['pts'].append([float(body[10]), float(body[20])])
            elif v == 'SEQEND' and poly is not None:
                if poly['closed']:
                    poly['pts'].append(poly['pts'][0])
                ents.append(PCurve('polyline', poly['layer'], poly['pts']))
                poly = None
            elif v == 'TEXT':
                ents.append(PCurve('text', body.get(8, '0'), [[float(body[10]), float(body[20])]] * 2))
                ents[-1].text = body.get(1, '')
            elif v in ('ENDSEC', 'EOF'):
                pass
            else:
                ents.append(PCurve('unknown:' + v, body.get(8, '0'), [[0, 0], [0, 0]]))
        i = j
    return ents, meta


# ------------------------------------------------------------------ parts
def split_grouped_outline(ents, curves):
    """Engine 0.4.5 (work plan task 1.3, RB-008): a part made of lines and polylines writes its outer profile as ONE
    closed polyline with the vertices of its curves, in order, each shared end point once. This function splits that
    polyline back into the curves the job declares, at the shared vertices, and checks every piece against that curve's
    own points in the result: a piece that does not match raises, so nothing is silently reinterpreted. Derivation: the
    grouped vertices V are the curve chain with the shared end points written once, so a line takes the next two
    vertices and a polyline takes its own count of vertices, sharing its end with the next curve; the walk must close
    on the first vertex. Any other entity list is returned unchanged (arcs are never grouped)."""
    outer = [i for i, e in enumerate(ents) if e.layer == 'CUT_OUTER']
    if len(outer) != 1 or ents[outer[0]].kind != 'polyline' or len(curves) < 2:
        return ents
    if not all(c['kind'] in ('line', 'polyline') for c in curves):
        return ents
    V = [tuple(p) for p in ents[outer[0]].pts]
    if len(V) > 1 and V[0] == V[-1]:
        V = V[:-1]
    n, pos, pieces = len(V), 0, []
    for c in curves:
        m = 2 if c['kind'] == 'line' else len(c['points'])
        idx = [(pos + k) % n for k in range(m)] if c['kind'] != 'line' else [pos, (pos + 1) % n]
        pos = (pos + m - 1) % n
        pts = np.asarray([V[k] for k in idx], float)
        want = np.asarray(c['points'], float)
        if pts.shape != want.shape or not np.allclose(pts, want, rtol=0, atol=1e-9):
            raise ValueError(f"grouped outer profile: curve {c['name']} does not match its own points")
        pieces.append(PCurve(c['kind'], 'CUT_OUTER', pts.tolist(), name=c['name']))
    if pos != 0:
        raise ValueError('grouped outer profile does not close on its first vertex')
    return ents[:outer[0]] + pieces + ents[outer[0] + 1:]


class Part:
    def __init__(self, view, dxf_text):
        self.view = view
        self.name = view['part']
        self.family = view['family']
        self.q = view['quantities']
        self.eps_num, self.eps_rep = view['eps_num'], view['eps_rep']
        self.dxf_text = dxf_text
        self.all_min, self.meta_min = read_dxf_minimal(dxf_text)
        self.all, self.meta = (read_dxf_ezdxf(dxf_text) if HAVE_EZDXF else (self.all_min, self.meta_min))
        self.all_min = split_grouped_outline(self.all_min, view['curves'])
        self.all = split_grouped_outline(self.all, view['curves'])
        # geometry that would be cut: everything on a CUT_* layer.  Anything else is kept separately and policed
        # by the round-trip check (only non-cutting TEXT on NF_NOT_RELEASED is tolerated).
        is_cut = lambda e: e.layer.startswith('CUT')
        self.ents = [e for e in self.all if is_cut(e)]
        self.ents_min = [e for e in self.all_min if is_cut(e)]
        self.other = [e for e in self.all if not is_cut(e)]
        jc = view['curves'] + view['holes']
        self.n_outline = len(view['curves'])
        for e, c in zip(self.ents, jc):
            e.name = c['name']
        self.outline = self.ents[:self.n_outline]
        self.holes = self.ents[self.n_outline:]

    def curve(self, name):
        for e in self.ents:
            if e.name == name:
                return e
        raise KeyError(name)

    def corner(self, n1, n2):
        a, b = self.curve(n1), self.curve(n2)
        best = min(((np.linalg.norm(p - q), p, q) for p in (a.start, a.end) for q in (b.start, b.end)), key=lambda x: x[0])
        return 0.5 * (best[1] + best[2])

    def extent(self):
        v = np.concatenate([e.pts for e in self.ents])
        return float(np.abs(v).max())

    # ---- measures computed independently from the DXF entities
    def loop_area(self, curves):
        """Signed area of a chain of curves (exact for arcs), following the chain head-to-tail."""
        if len(curves) == 1:
            return curves[0].area_term()
        total, pos = 0.0, None
        first = curves[0]
        nxt = curves[1]
        d_end = min(np.linalg.norm(first.end - nxt.start), np.linalg.norm(first.end - nxt.end))
        d_start = min(np.linalg.norm(first.start - nxt.start), np.linalg.norm(first.start - nxt.end))
        rev = d_start < d_end
        for c in curves:
            if pos is not None:
                rev = np.linalg.norm(c.end - pos) < np.linalg.norm(c.start - pos)
            total += c.area_term(reverse=rev)
            pos = c.start if rev else c.end
        return total

    def measures(self):
        m = {}
        m['outline_area'] = abs(self.loop_area(self.outline))
        m['outline_signed_area'] = self.loop_area(self.outline)
        m['outline_perimeter'] = sum(c.length() for c in self.outline)
        bb = np.array([c.bbox() for c in self.outline])
        m['bbox_xmin'], m['bbox_ymin'], m['bbox_xmax'], m['bbox_ymax'] = bb[:, 0].min(), bb[:, 1].min(), bb[:, 2].max(), bb[:, 3].max()
        m['bbox_w'], m['bbox_h'] = m['bbox_xmax'] - m['bbox_xmin'], m['bbox_ymax'] - m['bbox_ymin']
        m['hole_count'] = len(self.holes)
        if self.holes:
            h = self.holes[0]
            m['hole_signed_area'] = h.area_term()
            m['hole_area'] = abs(m['hole_signed_area'])
            m['hole_perimeter'] = h.length()
            m['hole_xmin'], m['hole_ymin'], m['hole_xmax'], m['hole_ymax'] = h.bbox()
            m['hole_w'], m['hole_h'] = m['hole_xmax'] - m['hole_xmin'], m['hole_ymax'] - m['hole_ymin']
            cen = shapely.Polygon(h.pts).centroid
            m['hole_cx'], m['hole_cy'] = cen.x, cen.y
        m['net_area'] = m['outline_area'] - m.get('hole_area', 0.0)
        for c in self.ents:
            m['len_' + c.name] = c.length()
        return {k: float(v) for k, v in m.items()}


class Job:
    def __init__(self, client, submitted):
        self.http = submitted['http']
        self.id = submitted['id']
        self.error = submitted['error']
        self.result = submitted['result']
        self.parts = {}
        self.dxf_status = {}
        r = self.result
        self.outcome = r.get('outcome') if r else None
        self.state = r.get('state') if r else None
        self.engine = r.get('engine') if r else None
        # a job whose own gates REJECT it withholds its files: for the caller that is a refusal
        self.self_rejected = bool(r and r.get('patterns') and r.get('state') == 'REJECTED')
        self.accepted = bool(r and r.get('patterns')) and not self.self_rejected
        # the stable refusal code (engine 0.4.1 and later), when the engine refused the request
        self.refusal_code = submitted.get('code') or ((r or {}).get('refusal') or {}).get('code')
        if r and self.accepted:
            for pv in r['patterns']:
                st, text = client.dxf(self.id, pv['part'])
                self.dxf_status[pv['part']] = st
                if st == 200:
                    self.parts[pv['part']] = Part(pv, text)

    @property
    def refused(self):
        return not self.accepted

    def refusal_text(self):
        if self.self_rejected:
            fails = [g['id'] + ' ' + g['name'] for g in self.result.get('gates', []) if g['result'] == 'FAIL']
            return 'pattern computed, then REJECTED by the product\'s own gates: ' + ', '.join(fails)
        return self.outcome or self.error or ''

    def checks(self):
        return self.result.get('checks', []) if self.result else []
