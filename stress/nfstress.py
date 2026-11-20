"""NEXUS FABRIC stress framework: server lifecycle, sampler, HTTP client, stats, integrity checks.
Python 3 stdlib + psutil. Never touches anything but the server instance it starts itself."""
import re, http.client, json, os, random, shutil, signal, sqlite3, subprocess, threading, time, hashlib, socket
import psutil

HERE = os.path.dirname(os.path.abspath(__file__))
ADMIN = ("admin", "correct-horse-9")
FAMILIES = {
    "cylinder_shell": lambda k=1: {"family": "cylinder_shell", "r_dev": 500 + k * 0.001, "length": 1000},
    "cone_frustum": lambda k=1: {"family": "cone_frustum", "r1": 200 + k * 0.001, "r2": 100, "h": 300},
    "eccentric_reducer": lambda k=1: {"family": "eccentric_reducer", "r1": 200 + k * 0.001, "r2": 100, "h": 300, "e": 100},
    "branch_stub": lambda k=1: {"family": "branch_stub", "header_radius": 150 + k * 0.001, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": 400},
    "header_with_branch": lambda k=1: {"family": "header_with_branch", "header_radius": 150 + k * 0.001, "header_dev_radius": 150, "header_length": 1200, "branch_station": 600,
                                       "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": 400, "surface_reference": "outside"},
}
DXF_PART = {"cylinder_shell": "shell", "cone_frustum": "reducer", "eccentric_reducer": "reducer", "branch_stub": "branch", "header_with_branch": "header"}


def pct(vals, p):
    if not vals:
        return None
    s = sorted(vals)
    return s[min(len(s) - 1, int(round(p / 100.0 * (len(s) - 1))))]


# One-time setup token of the disposable servers this runner starts (the server refuses first-run setup without it).
SETUP_TOKEN = "stress-runner-setup-token-0123456789"

class Recorder:
    """Thread-safe per-request record: (t, latency_s, status, label)."""
    def __init__(self):
        self.lock = threading.Lock()
        self.rows = []
        self.bad = []  # 5xx / connection errors with repro info

    def add(self, t, lat, status, label, repro=None):
        with self.lock:
            self.rows.append((t, lat, status, label))
            if (status >= 500 or status == 0) and len(self.bad) < 200:
                self.bad.append({"label": label, "status": status, "repro": repro})

    def summary(self, rows=None, dur=None):
        rows = self.rows if rows is None else rows
        lats = [r[1] * 1000 for r in rows]
        n = len(rows)
        if not n:
            return {"requests": 0}
        t0 = min(r[0] for r in rows); t1 = max(r[0] + r[1] for r in rows)
        dur = dur or max(t1 - t0, 1e-9)
        e5 = sum(1 for r in rows if r[2] >= 500 or r[2] == 0)
        return {"requests": n, "errors_5xx_or_conn": e5, "error_rate": round(e5 / n, 5),
                "p50_ms": round(pct(lats, 50), 2), "p95_ms": round(pct(lats, 95), 2), "p99_ms": round(pct(lats, 99), 2),
                "max_ms": round(max(lats), 2), "throughput_rps": round(n / dur, 1), "duration_s": round(dur, 1)}

    def by_label(self):
        d = {}
        for r in self.rows:
            d.setdefault(r[3], []).append(r)
        return {k: self.summary(v) for k, v in sorted(d.items())}


class Client:
    def __init__(self, port, rec=None, timeout=60):
        self.port, self.rec, self.timeout = port, rec, timeout
        self.cookie = None
        self.conn = None

    def _c(self):
        if self.conn is None:
            self.conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=self.timeout)
        return self.conn

    def req(self, method, path, body=None, label=None, raw=False, headers=None, csrf=True):
        h = {}
        if csrf and method != "GET":
            h["X-NF-Request"] = "1"
        if self.cookie:
            h["Cookie"] = self.cookie
        data = None
        if body is not None:
            if isinstance(body, (bytes, bytearray)):
                data = bytes(body)
            else:
                data = json.dumps(body).encode(); h["Content-Type"] = "application/json"
        if headers:
            h.update(headers)
        t = time.time(); status = 0; payload = b""; rh = {}
        for attempt in (0, 1):
            try:
                c = self._c()
                c.request(method, path, body=data, headers=h)
                r = c.getresponse(); payload = r.read(); status = r.status; rh = dict(r.getheaders())
                if r.getheader("Connection", "").lower() == "close":
                    c.close(); self.conn = None
                break
            except (http.client.HTTPException, OSError) as e:
                try:
                    self.conn.close()
                except Exception:
                    pass
                self.conn = None
                status = 0; payload = repr(e).encode()
                if attempt == 0 and isinstance(e, (http.client.RemoteDisconnected, BrokenPipeError, ConnectionResetError)) and time.time() - t < 1:
                    continue  # stale keep-alive connection; retry once
                break
        lat = time.time() - t
        if self.rec is not None:
            self.rec.add(t, lat, status, label or (method + " " + re.sub(r"\d+", "N", path.split("?")[0])[:60]),
                         {"method": method, "path": path, "body": (body if not isinstance(body, (bytes, bytearray)) else f"<{len(body)} bytes>") if (body is None or len(str(body)) < 2000) else "<large>", "resp": payload[:200].decode("latin1")})
        if raw:
            return status, payload, rh
        try:
            js = json.loads(payload) if payload else None
        except Exception:
            js = None
        return status, js

    def login(self, user=ADMIN[0], pw=ADMIN[1], label=None):
        c = self._c()
        t = time.time()
        d = json.dumps({"username": user, "password": pw}).encode()
        try:
            c.request("POST", "/api/login", body=d, headers={"X-NF-Request": "1", "Content-Type": "application/json"})
            r = c.getresponse(); r.read(); status = r.status
            sc = r.getheader("Set-Cookie")
            if sc:
                self.cookie = sc.split(";")[0]
        except Exception as e:
            status = 0; self.conn = None
        if self.rec is not None:
            self.rec.add(t, time.time() - t, status, label or "login", {"path": "/api/login"})
        return status


_LIVE = []


def _reap():
    for p in _LIVE:
        if p.poll() is None:
            p.kill()


import atexit
atexit.register(_reap)


class Server:
    def __init__(self, workdir, port, binary=None, name="srv", data_dir=None, key_file=None, keep=False):
        self.binary = binary or os.environ.get("NF_BIN") or os.path.join(os.path.dirname(HERE), "target", "release", "nf-server")
        self.port, self.workdir, self.name = port, workdir, name
        self.data = data_dir or os.path.join(workdir, f"data-{name}")
        self.key_file = key_file
        self.proc = None; self.ps = None; self.samples = []; self._stop = threading.Event(); self.log = os.path.join(workdir, f"{name}.log")
        os.makedirs(workdir, exist_ok=True)

    def env(self):
        e = dict(os.environ, NF_DATA_DIR=self.data, NF_LISTEN=f"127.0.0.1:{self.port}", NF_SETUP_TOKEN=SETUP_TOKEN)
        if self.key_file:
            e["NF_KEY_FILE"] = self.key_file
        return e

    def start(self, wait=15):
        try:
            socket.create_connection(("127.0.0.1", self.port), timeout=0.3).close()
            raise RuntimeError(f"port {self.port} already in use: refusing to start (and never talk to a server we did not start)")
        except OSError:
            pass
        os.makedirs(self.data, exist_ok=True)
        lf = open(self.log, "ab")
        self.proc = subprocess.Popen([self.binary], env=self.env(), stdout=lf, stderr=lf, stdin=subprocess.DEVNULL)
        self.ps = psutil.Process(self.proc.pid)
        _LIVE.append(self.proc)
        t0 = time.time()
        while time.time() - t0 < wait:
            if self.proc.poll() is not None:
                return False
            try:
                s = socket.create_connection(("127.0.0.1", self.port), timeout=0.5); s.close()
                st, _ = Client(self.port, timeout=5).req("GET", "/api/health")
                if st == 200:
                    return True
            except OSError:
                time.sleep(0.05)
        return False

    def alive(self):
        return self.proc is not None and self.proc.poll() is None

    def sigterm(self, wait=10):
        if self.alive():
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(wait)
            except subprocess.TimeoutExpired:
                return False
        return True

    def kill9(self):
        if self.alive():
            self.proc.kill(); self.proc.wait()

    def stop(self):
        self.stop_sampler()
        if self.alive():
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(8)
            except subprocess.TimeoutExpired:
                self.proc.kill(); self.proc.wait()

    # sampler
    def dir_size(self):
        tot = 0
        for r, _, fs in os.walk(self.data):
            for f in fs:
                try:
                    tot += os.path.getsize(os.path.join(r, f))
                except OSError:
                    pass
        return tot

    def start_sampler(self, interval=1.0):
        self._stop.clear(); self.samples = []
        def run():
            try:
                self.ps.cpu_percent(None)
            except Exception:
                return
            t0 = time.time()
            while not self._stop.wait(interval):
                try:
                    with self.ps.oneshot():
                        self.samples.append({"t": round(time.time() - t0, 1), "cpu": self.ps.cpu_percent(None), "rss_mb": round(self.ps.memory_info().rss / 1048576, 1),
                                             "threads": self.ps.num_threads(), "fds": self.ps.num_fds(), "data_mb": round(self.dir_size() / 1048576, 1)})
                except Exception:
                    pass
        self._th = threading.Thread(target=run, daemon=True); self._th.start()

    def stop_sampler(self):
        self._stop.set()

    def sample_summary(self):
        s = self.samples
        if not s:
            return {}
        return {"peak_rss_mb": max(x["rss_mb"] for x in s), "peak_cpu_pct": max(x["cpu"] for x in s), "mean_cpu_pct": round(sum(x["cpu"] for x in s) / len(s), 1),
                "peak_threads": max(x["threads"] for x in s), "peak_fds": max(x["fds"] for x in s), "final_data_mb": s[-1]["data_mb"]}


def fresh_admin(srv):
    c = Client(srv.port)
    st, _ = c.req("POST", "/api/setup", {"username": ADMIN[0], "password": ADMIN[1], "setup_token": SETUP_TOKEN})
    c.login()
    return c


def linfit(xs, ys):
    n = len(xs)
    if n < 3:
        return None
    mx = sum(xs) / n; my = sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        return None
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    a = my - b * mx
    ss_tot = sum((y - my) ** 2 for y in ys) or 1e-12
    ss_res = sum((y - (a + b * x)) ** 2 for x, y in zip(xs, ys))
    return {"slope": b, "intercept": a, "r2": 1 - ss_res / ss_tot}


# ---------- integrity ----------
def _fl(f):
    r = repr(f)
    if "e" in r:
        m, e = r.split("e"); sign = "-" if e.startswith("-") else ""; e = e.lstrip("+-").lstrip("0") or "0"
        return f"{m}e{sign}{e}"
    return r


def _canon(v):
    """serde_json compact form: sorted keys, ryu-style float exponents."""
    if isinstance(v, dict):
        return "{" + ",".join(json.dumps(k, ensure_ascii=False) + ":" + _canon(v[k]) for k in sorted(v)) + "}"
    if isinstance(v, list):
        return "[" + ",".join(_canon(x) for x in v) + "]"
    if isinstance(v, float):
        return _fl(v)
    return json.dumps(v, ensure_ascii=False)


def integrity_check(data_dir, srv_port=None, client=None, replay_sample=60):
    """Opens the SQLite file directly (server should be stopped or idle). Returns dict of findings."""
    res = {}
    db = sqlite3.connect(f"file:{os.path.join(data_dir, 'nexus-fabric.sqlite')}?mode=ro", uri=True, timeout=30)
    res["integrity_check"] = db.execute("PRAGMA integrity_check").fetchall()
    res["integrity_check"] = [r[0] for r in res["integrity_check"]]
    for t in ("users", "projects", "calculations", "records", "audit", "obligations", "documents", "lessons", "sessions"):
        res[f"n_{t}"] = db.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0]
    # audit chain
    prev = "genesis"; broken = None; ids_gap = 0; last_id = 0
    for (i, at, user, action, detail, p, h) in db.execute("SELECT id,at,username,action,detail,prev,hash FROM audit ORDER BY id"):
        exp = hashlib.sha256(json.dumps([prev, at, user, action, detail], separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
        if i != last_id + 1:
            ids_gap += 1
        last_id = i
        if p != prev or h != exp:
            broken = i; break
        prev = exp
    res["audit_chain_python"] = {"broken_at": broken, "id_gaps": ids_gap}
    # duplicate/lost jobs: ids contiguous, every calc has result_json parseable & non-empty closure hash
    bad_calc = 0; seen_ids = []
    for (i, state, ch, rj, fam) in db.execute("SELECT id,state,closure_hash,result_json,family FROM calculations ORDER BY id"):
        seen_ids.append(i)
        try:
            j = json.loads(rj)
            if not ch or j.get("closure_hash") != ch or state not in ("REVIEW_REQUIRED", "REJECTED", "REFUSED", "APPROVED", "REVIEW_REQUIRED "):  # REFUSED: input refused, nothing computed (a valid recorded outcome)
                bad_calc += 1
        except Exception:
            bad_calc += 1
    res["calc_bad_rows"] = bad_calc
    res["calc_id_gaps"] = (seen_ids[-1] - len(seen_ids)) if seen_ids else 0
    # orphan checks
    res["orphan_calc_project"] = db.execute("SELECT COUNT(*) FROM calculations WHERE project_id NOT IN (SELECT id FROM projects)").fetchone()[0]
    res["calc_missing_blob"] = 0
    missing = 0
    for (rj,) in db.execute("SELECT result_json FROM calculations ORDER BY id DESC LIMIT 400"):
        try:
            for p in json.loads(rj).get("patterns", []):
                s = p.get("dxf_sha256")
                if s and not os.path.exists(os.path.join(data_dir, "blobs", s + ".bin")):
                    missing += 1
        except Exception:
            pass
    res["calc_missing_blob"] = missing
    # record identity hash (python canonical; mismatches are 'suspect' until cross-checked by replay endpoint)
    suspect = []; total = 0
    for (rid, rj) in db.execute("SELECT id,json FROM records"):
        total += 1
        try:
            r = json.loads(rj)
            body = {"family": r["family"], "kind": r["kind"], "payload": r["payload"], "relations": r["relations"], "activity": r.get("activity")}
            if hashlib.sha256(_canon(body).encode()).hexdigest() != rid or r["id"] != rid:
                suspect.append(rid)
        except Exception:
            suspect.append(rid)
    res["records_total"] = total
    res["records_suspect_python_hash"] = len(suspect)
    owners = set()
    for sid in suspect[:200]:
        row = db.execute("SELECT id FROM calculations WHERE result_json LIKE ? LIMIT 1", ("%" + sid + "%",)).fetchone()
        if row: owners.add(row[0])
    res["suspect_owner_calcs"] = sorted(owners)
    db.close()
    res["suspect_ids"] = suspect[:200]
    return res


def replay_sample(client, n=40, maxid=None):
    """Authoritative identity check via the server's replay (recomputes record identity in the server)."""
    st, d = client.req("GET", "/api/calculations")
    ids = [c["id"] for c in d] if st == 200 and d else []
    random.shuffle(ids)
    bad = 0; checked = 0; refused = 0; errs = []
    for cid in ids[:n]:
        v = replay_verdict(client, cid, label="replay")
        if v == "refused":
            refused += 1; continue
        checked += 1
        if v != "ok":
            bad += 1; errs.append((cid, v))
    return {"checked": checked, "bad": bad, "refused_nothing_to_replay": refused, "errs": errs[:5]}


def replay_verdict(client, cid, label=None):
    """'ok' if the server's replay matches; 'refused' if the job was a refused request (nothing was computed, so there is
    nothing to replay: the server answers 409 and the job's state is REFUSED); otherwise a description of the failure."""
    st, r = client.req("POST", f"/api/calculations/{cid}/replay", label=label)
    if st == 200 and r and r.get("matches") and not r.get("stored_records_failing_identity_check", 0):
        return "ok"
    if st == 409:
        s2, job = client.req("GET", f"/api/calculations/{cid}")
        if s2 == 200 and job and job.get("state") == "REFUSED":
            return "refused"
    return f"{st} {str(r)[:200]}"


def machine():
    return {"cores": os.cpu_count(), "ram_mb": psutil.virtual_memory().total // 1048576, "loadavg": os.getloadavg()}
