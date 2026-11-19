#!/usr/bin/env python3
"""NEXUS FABRIC stress runner.
  python3 stress/run.py --level N --duration S [--port 8810] [--bin PATH] [--workdir DIR] [--url http://127.0.0.1:PORT]
Starts and stops its own disposable server instance (NF_DATA_DIR under --workdir). --url only selects the port (127.0.0.1
only); the runner never talks to a server it did not start. Results: stress/results/levelN-<ts>.json and .md"""
import argparse, json, os, random, shutil, signal, socket, subprocess, sys, threading, time, tempfile, resource
from urllib.parse import urlparse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from nfstress import *

RESULTS = os.path.join(HERE, "results")
STOP = threading.Event()


def parallel(n, fn, *a):
    ths = [threading.Thread(target=fn, args=(i,) + a, daemon=True) for i in range(n)]
    [t.start() for t in ths]
    return ths


def join(ths):
    [t.join() for t in ths]


def dirsize(p):
    tot = 0
    for r, _, fs in os.walk(p):
        for f in fs:
            try:
                tot += os.path.getsize(os.path.join(r, f))
            except OSError:
                pass
    return tot


def new_engineer(admin, name, pw="engineer-pass-1", role="engineer"):
    return admin.req("POST", "/api/users", {"username": name, "password": pw, "role": role})


def workflow_once(c, pid, fam, k, check):
    st, d = c.req("POST", f"/api/projects/{pid}/calculations", FAMILIES[fam](k), label=f"calc:{fam}")
    if st != 200:
        check(f"calc {fam} status {st}", False); return None
    cid = d["id"]
    c.req("GET", f"/api/calculations/{cid}", label="get_calc")
    st, body, _ = c.req("GET", f"/api/calculations/{cid}/dxf/{DXF_PART[fam]}", raw=True, label="dxf_download")
    if fam == "header_with_branch":
        c.req("GET", f"/api/calculations/{cid}/dxf/branch", raw=True, label="dxf_download")
        c.req("GET", f"/api/calculations/{cid}/dependencies", label="dependencies")
    return cid


# ======================= LEVEL 1 =======================
def level1(ctx):
    srv, rec = ctx["srv"], ctx["rec"]
    c = Client(srv.port, rec); c.cookie = ctx["admin"].cookie
    st, d = c.req("POST", "/api/projects", {"name": "L1 baseline", "description": "single-user workflow"}, label="create_project"); pid = d["id"]
    t_end = time.time() + ctx["duration"]; k = 0; fails = []
    while time.time() < t_end:
        for fam in FAMILIES:
            k += 1
            cid = workflow_once(c, pid, fam, k, lambda m, ok: ok or fails.append(m))
            if cid and k % 5 == 0:
                c.req("POST", f"/api/calculations/{cid}/replay", label="replay")
            if cid and fam == "header_with_branch":
                st, d = c.req("POST", f"/api/calculations/{cid}/corrections", {"parameter": "header_radius", "new_value": 140 + k * 0.01, "reason": "baseline correction"}, label="correction")
                if st == 200:
                    c.req("GET", "/api/obligations", label="list_obligations")
        for ep in ("/api/calculations", "/api/projects", "/api/dashboard", f"/api/projects/{pid}", "/api/audit", "/api/audit/verify", "/api/lessons", "/api/amoeba/summary", "/api/me", "/api/health"):
            c.req("GET", ep, label="GET " + ep)
    return {"workload": "single admin user: project, 5 families (calc, get, DXF), replay every 5th, correction, listings, audit verify", "fails": fails}


# ======================= LEVEL 2 =======================
def level2(ctx):
    srv, rec, adm = ctx["srv"], ctx["rec"], ctx["admin"]
    out = {}
    port = srv.port
    new_engineer(adm, "eng2")
    c = Client(port, rec); c.login("eng2", "engineer-pass-1")
    st, d = c.req("POST", "/api/projects", {"name": "L2 heavy", "description": "x" * 250000}, label="create_project_250KB_description")
    out["project_250KB_description"] = st; pid = d["id"] if d else 1
    # (a) magnitude sweep
    mag = []
    for e in (1, 3, 6, 9, 12, 15, 100, 300, 308):
        v = 10.0 ** e
        for name, body in [("cylinder_shell", {"family": "cylinder_shell", "r_dev": v, "length": v}), ("cone_frustum", {"family": "cone_frustum", "r1": v, "r2": v / 2, "h": v}),
                           ("eccentric_reducer", {"family": "eccentric_reducer", "r1": v, "r2": v / 2, "h": v, "e": v / 4}),
                           ("branch_stub", {"family": "branch_stub", "header_radius": v * 1.5, "branch_radius": v, "branch_dev_radius": v, "beta_deg": 60, "delta": 0, "s_end": v * 4}),
                           ("header_with_branch", dict(FAMILIES["header_with_branch"](), header_radius=v * 1.5, header_dev_radius=v * 1.5, header_length=v * 12, branch_station=v * 6, branch_radius=v, branch_dev_radius=v, s_end=v * 4))]:
            t = time.time()
            st, r = c.req("POST", f"/api/projects/{pid}/calculations", body, label=f"heavy_calc_1e{e}")
            row = {"family": name, "mag": f"1e{e}", "status": st, "ms": round((time.time() - t) * 1000, 1)}
            if st == 200:
                row["state"] = r["state"]
                s2, g = c.req("GET", f"/api/calculations/{r['id']}")
                row["patterns"] = len(g.get("patterns", [])) if g else None
                if g and g.get("patterns"):
                    s3, b, _ = c.req("GET", f"/api/calculations/{r['id']}/dxf/{g['patterns'][0]['part']}", raw=True)
                    row["dxf_status"] = s3; row["dxf_bytes"] = len(b)
            else:
                row["err"] = (r or {}).get("error", "")[:100] if isinstance(r, dict) else ""
            mag.append(row)
    out["magnitude_sweep"] = mag
    # (b) many projects
    NP = ctx.get("n_projects", 1500)
    def mk(i):
        cc = Client(port, rec); cc.cookie = c.cookie
        for j in range(NP // 6):
            cc.req("POST", "/api/projects", {"name": f"proj-{i}-{j}", "description": "bulk"}, label="bulk_create_project")
    join(parallel(6, mk))
    for _ in range(10):
        c.req("GET", "/api/projects", label="list_projects_%d" % NP)
    # (c) many jobs in one project
    NJ = ctx.get("n_jobs", 3000); cnt = [0]; lock = threading.Lock()
    def jobs(i):
        cc = Client(port, rec); cc.cookie = c.cookie; fams = list(FAMILIES)
        while True:
            with lock:
                if cnt[0] >= NJ:
                    return
                cnt[0] += 1; k = cnt[0]
            cc.req("POST", f"/api/projects/{pid}/calculations", FAMILIES[fams[k % 5]](k), label="bulk_calc")
    t = time.time(); join(parallel(6, jobs)); out["bulk_jobs_s"] = round(time.time() - t, 1); out["bulk_jobs"] = NJ
    for _ in range(10):
        for ep in ("/api/calculations", "/api/dashboard", "/api/amoeba/summary", f"/api/projects/{pid}", "/api/projects", "/api/obligations", "/api/audit", "/api/audit/verify"):
            c.req("GET", ep, label="LISTING " + ep)
    # (d) DXF downloads + replays
    st, lst = c.req("GET", "/api/calculations")
    ids = [x["id"] for x in lst][:200]
    for cid in ids:
        c.req("GET", f"/api/calculations/{cid}", label="get_calc_large")
    for cid in ids[:100]:
        for part in ("shell", "reducer", "branch", "header"):
            c.req("GET", f"/api/calculations/{cid}/dxf/{part}", raw=True, label="dxf_download")
    for cid in ids[:25]:
        c.req("POST", f"/api/calculations/{cid}/replay", label="replay")
    out["jobs_per_project"] = NJ + len(mag)
    return out


# ======================= LEVEL 3 =======================
def raw_http(port, data, timeout=15, read=65536):
    s = socket.create_connection(("127.0.0.1", port), timeout=timeout)
    try:
        s.sendall(data)
        buf = b""
        try:
            while b"\r\n\r\n" not in buf:
                x = s.recv(read)
                if not x:
                    break
                buf += x
        except socket.timeout:
            return "TIMEOUT"
        except OSError as e:
            return f"ERR {e}"
        line = buf.split(b"\r\n", 1)[0].decode("latin1")
        return line or "EMPTY(closed)"
    except OSError as e:
        return f"ERR {e}"
    finally:
        s.close()


def big_post(port, path, size, cookie, extra=b"", prefix=b"%PDF-1.7\n", ctype=b"application/octet-stream"):
    """Send a large body over a raw socket; report the status line even if the server closes early."""
    s = socket.create_connection(("127.0.0.1", port), timeout=60); t = time.time()
    hdr = b"POST " + path.encode() + b" HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nCookie: " + cookie.encode() + b"\r\nContent-Type: " + ctype + b"\r\n" + extra + b"Content-Length: " + str(size).encode() + b"\r\n\r\n"
    sent = 0; err = None
    try:
        s.sendall(hdr); chunk = prefix + b"a" * (1 << 20)
        while sent < size:
            n = min(len(chunk), size - sent); s.sendall(chunk[:n]); sent += n
    except OSError as e:
        err = repr(e)
    try:
        s.settimeout(30); buf = s.recv(65536)
        line = buf.split(b"\r\n", 1)[0].decode("latin1") or "EMPTY"
    except OSError as e:
        line = f"NO RESPONSE {e}"
    s.close()
    return {"status_line": line, "bytes_sent": sent, "send_error": err, "ms": round((time.time() - t) * 1000)}


def level3(ctx):
    srv, rec, adm = ctx["srv"], ctx["rec"], ctx["admin"]
    port = srv.port; out = {}
    c = Client(port, rec); c.cookie = adm.cookie
    st, d = c.req("POST", "/api/projects", {"name": "L3 extreme"}); pid = d["id"]
    # ---- growth curve
    stages = ctx.get("stages", [1000, 3000, 6000, 10000, 15000, 20000]); disk_cap = ctx.get("disk_cap_mb", 4096)
    growth = []; cnt = [0]; lock = threading.Lock(); target = [0]; stop_growth = [False]
    def worker(i):
        cc = Client(port, rec); cc.cookie = c.cookie; fams = ["cone_frustum", "cylinder_shell", "eccentric_reducer", "branch_stub"]
        while not stop_growth[0]:
            with lock:
                if cnt[0] >= target[0]:
                    return
                cnt[0] += 1; k = cnt[0]
            cc.req("POST", f"/api/projects/{pid}/calculations", FAMILIES[fams[k % 4]](k), label="growth_calc")
    for tgt in stages:
        target[0] = tgt; t = time.time(); n0 = cnt[0]
        join(parallel(5, worker)); dt = time.time() - t
        row = {"jobs": cnt[0], "create_rate_per_s": round((cnt[0] - n0) / max(dt, 1e-9), 1), "data_mb": round(srv.dir_size() / 1048576, 1)}
        for name, ep in (("list_calcs", "/api/calculations"), ("dashboard", "/api/dashboard"), ("amoeba", "/api/amoeba/summary"), ("list_projects", "/api/projects"), ("get_project", f"/api/projects/{pid}"), ("health", "/api/health")):
            t = time.time(); s, b, _ = c.req("GET", ep, raw=True); row[name + "_ms"] = round((time.time() - t) * 1000, 1); row[name + "_status"] = s
        t = time.time(); s, _ = c.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["cone_frustum"](-cnt[0])); row["single_calc_ms"] = round((time.time() - t) * 1000, 1)
        t = time.time(); s, _ = c.req("GET", "/api/audit/verify"); row["audit_verify_ms"] = round((time.time() - t) * 1000, 1)
        row["rss_mb"] = srv.samples[-1]["rss_mb"] if srv.samples else None
        growth.append(row)
        if row["data_mb"] > disk_cap:
            row["stopped"] = "disk cap %d MB" % disk_cap; break
        if row["list_calcs_ms"] > 20000 or row["amoeba_ms"] > 30000:
            row["stopped"] = "latency > 20 s"; break
    out["growth"] = growth
    # ---- limits
    lim = {}
    def probe(name, method, path, body=None, raw_body=None, headers=None):
        t = time.time()
        if raw_body is not None:
            st, b, h = c.req(method, path, raw_body, raw=True, headers=headers or {"Content-Type": "application/json"}, label="limit:" + name)
            msg = b[:80].decode("latin1")
            try: r = json.loads(b)
            except Exception: r = None
        else:
            st, r = c.req(method, path, body, label="limit:" + name); msg = json.dumps(r)[:80] if r is not None else ""
        lim[name] = {"status": st, "ms": round((time.time() - t) * 1000), "msg": msg}
        if st == 200 and path.endswith("/calculations") and r is not None:
            s2, g = c.req("GET", f"/api/calculations/{r['id']}")
            if g:
                lim[name]["patterns"] = len(g.get("patterns", []))
                if g.get("patterns") and g["state"] != "REJECTED":
                    s3, b, _ = c.req("GET", f"/api/calculations/{r['id']}/dxf/{g['patterns'][0]['part']}", raw=True); lim[name]["dxf_status"] = s3; lim[name]["dxf_bytes"] = len(b)
        return st
    P = f"/api/projects/{pid}/calculations"; cyl = FAMILIES["cylinder_shell"]()
    for n in (200, 201, 100000, 262144):
        probe(f"project_name_{n}", "POST", "/api/projects", {"name": "n" * n})
    probe("project_description_200KB", "POST", "/api/projects", {"name": "d", "description": "d" * 200000})
    probe("project_name_unicode_200_emoji", "POST", "/api/projects", {"name": "\U0001F600" * 200})
    probe("project_name_NUL_and_control", "POST", "/api/projects", {"name": "a\u0000b\u0007c\n"})
    lim["body_10MB_raw"] = big_post(port, "/api/projects", 10 * 1024 * 1024, c.cookie, ctype=b"application/json", prefix=b'{"name":"b","description":"')
    lim["body_100MB_raw"] = big_post(port, "/api/projects", 100 * 1024 * 1024, c.cookie, ctype=b"application/json", prefix=b'{"name":"b","description":"')
    for sz in (262144 - 200, 262144 + 1, 1024 * 1024):
        pad = "x" * sz
        probe(f"body_{sz}B", "POST", "/api/projects", raw_body=('{"name":"b","description":"' + pad + '"}').encode())
    probe("surface_reference_200KB", "POST", P, dict(FAMILIES["header_with_branch"](), surface_reference="s" * 200000))
    probe("surface_reference_empty", "POST", P, dict(FAMILIES["header_with_branch"](), surface_reference=""))
    for dep in (50, 120, 127, 129, 1000, 100000):
        probe(f"json_depth_{dep}", "POST", P, raw_body=(b'{"a":' * dep + b'1' + b'}' * dep))
    probe("json_array_depth_100000", "POST", P, raw_body=b"[" * 100000 + b"]" * 100000)
    probe("json_100k_keys", "POST", P, raw_body=("{" + ",".join(f'"k{i}":1' for i in range(30000)) + "}").encode())
    probe("json_dup_keys", "POST", P, raw_body=b'{"family":"cylinder_shell","r_dev":1,"r_dev":2,"length":3}')
    for v in ("1e308", "1.7976931348623157e308", "1e309", "-1e309", "4.9e-324", "1e-400", "0", "-0", "-5", "0.000001", "123456789012345678901234567890", "NaN", "Infinity", "null", "\"12\"", "true", "[]", "{}"):
        probe(f"num_r_dev_{v}", "POST", P, raw_body=('{"family":"cylinder_shell","r_dev":%s,"length":1000}' % v).encode())
    for fam, body in (("cone_frustum_r1_eq_r2", {"family": "cone_frustum", "r1": 100, "r2": 100, "h": 300}), ("cone_h0", {"family": "cone_frustum", "r1": 100, "r2": 50, "h": 0}), ("cone_neg", {"family": "cone_frustum", "r1": -100, "r2": 50, "h": 30}),
                      ("ecc_e_huge", {"family": "eccentric_reducer", "r1": 100, "r2": 50, "h": 30, "e": 1e300}), ("stub_beta_0", {"family": "branch_stub", "header_radius": 150, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 0, "delta": 0, "s_end": 400}),
                      ("stub_beta_180", {"family": "branch_stub", "header_radius": 150, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 180, "delta": 0, "s_end": 400}),
                      ("stub_branch_gt_header", {"family": "branch_stub", "header_radius": 50, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": 400}),
                      ("stub_beta_1e300", {"family": "branch_stub", "header_radius": 150, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 1e300, "delta": 0, "s_end": 400}),
                      ("stub_delta_1e300", {"family": "branch_stub", "header_radius": 150, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 1e300, "s_end": 400}),
                      ("stub_s_end_neg", {"family": "branch_stub", "header_radius": 150, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": -400}),
                      ("stub_tiny_all", {"family": "branch_stub", "header_radius": 1e-300, "branch_radius": 1e-301, "branch_dev_radius": 1e-301, "beta_deg": 60, "delta": 0, "s_end": 1e-300}),
                      ("cylinder_tiny", {"family": "cylinder_shell", "r_dev": 1e-300, "length": 1e-300}), ("unknown_family", {"family": "teapot"}), ("extra_fields", dict(cyl, extra="x" * 1000))):
        probe(fam, "POST", P, body)
    for name, path in (("calc_id_huge", "/api/calculations/99999999999999999999"), ("calc_id_neg", "/api/calculations/-1"), ("calc_id_text", "/api/calculations/abc"), ("calc_id_0", "/api/calculations/0"),
                       ("dxf_part_long", "/api/calculations/1/dxf/" + "p" * 60000), ("dxf_part_traversal", "/api/calculations/1/dxf/..%2f..%2fetc%2fpasswd"), ("url_100KB", "/api/" + "a" * 100000), ("project_id_huge", "/api/projects/9223372036854775808")):
        probe(name, "GET", path)
    # correction extremes
    for name, body in (("corr_1e308", {"parameter": "header_radius", "new_value": 1e308, "reason": "extreme test"}), ("corr_reason_100KB", {"parameter": "header_radius", "new_value": 151, "reason": "r" * 100000}),
                       ("corr_param_unknown", {"parameter": "nope", "new_value": 1, "reason": "unknown param"}), ("corr_neg", {"parameter": "header_radius", "new_value": -5, "reason": "negative test"})):
        hid = c.req("POST", P, FAMILIES["header_with_branch"](random.random()))[1]["id"]
        probe(name, "POST", f"/api/calculations/{hid}/corrections", body)
    # users / login
    for n in (1, 100, 5000, 200000):
        probe(f"username_len_{n}", "POST", "/api/users", {"username": "u" * n, "password": "long-enough-pass", "role": "viewer"})
    for n in (10, 1000, 100000, 250000):
        probe(f"password_len_{n}", "POST", "/api/users", {"username": f"pw{n}", "password": "p" * n, "role": "viewer"})
    probe("login_password_250KB", "POST", "/api/login", {"username": "admin", "password": "p" * 250000})
    # uploads
    lim["upload_52MB+2KB_raw"] = big_post(port, f"/api/projects/{pid}/documents", 50 * 1024 * 1024 + 2048, c.cookie, extra=b"X-Filename: z.pdf\r\n")
    lim["upload_60MB_raw"] = big_post(port, f"/api/projects/{pid}/documents", 60 * 1024 * 1024, c.cookie, extra=b"X-Filename: z.pdf\r\n")
    lim["upload_300MB_raw"] = big_post(port, f"/api/projects/{pid}/documents", 300 * 1024 * 1024, c.cookie, extra=b"X-Filename: z.pdf\r\n")
    for sz in (1, 1024 * 1024, 10 * 1024 * 1024, 50 * 1024 * 1024, 50 * 1024 * 1024 + 1):
        data = b"%PDF-1.7\n" + os.urandom(1024)[:1] * (sz - 9) if sz > 9 else b"%"
        t = time.time(); st, b, h = c.req("POST", f"/api/projects/{pid}/documents", data, raw=True, headers={"X-Filename": "big.pdf", "Content-Type": "application/octet-stream"}, label="limit:upload")
        lim[f"upload_{sz}B"] = {"status": st, "ms": round((time.time() - t) * 1000), "rss_mb_after": psutil.Process(srv.proc.pid).memory_info().rss // 1048576, "msg": b[:80].decode("latin1")}
        if sz >= 50 * 1024 * 1024 - 1:
            time.sleep(0.2)
    out["limits"] = lim
    # during-upload stall: health latency while a 50MB upload is in flight
    data = b"%PDF-1.7\n" + b"z" * (49 * 1024 * 1024)
    lat = []; done = threading.Event()
    def up():
        cc = Client(port, rec); cc.cookie = c.cookie
        cc.req("POST", f"/api/projects/{pid}/documents", data + os.urandom(8), raw=True, label="upload_49MB", headers={"X-Filename": "s.pdf"}); done.set()
    th = threading.Thread(target=up); th.start()
    while not done.is_set():
        t = time.time(); c.req("GET", "/api/health", label="health_during_upload"); lat.append((time.time() - t) * 1000); time.sleep(0.02)
    th.join()
    out["health_latency_during_49MB_upload_ms"] = {"n": len(lat), "p50": round(pct(lat, 50), 1), "max": round(max(lat), 1)}
    # raw protocol limits
    raw = {}
    raw["header_8KB"] = raw_http(port, b"GET /api/health HTTP/1.1\r\nHost: x\r\nX-A: " + b"a" * 8000 + b"\r\n\r\n")
    raw["header_64KB"] = raw_http(port, b"GET /api/health HTTP/1.1\r\nHost: x\r\nX-A: " + b"a" * 65000 + b"\r\n\r\n")
    raw["header_1MB"] = raw_http(port, b"GET /api/health HTTP/1.1\r\nHost: x\r\nX-A: " + b"a" * 1000000 + b"\r\n\r\n")
    raw["header_5000_headers"] = raw_http(port, b"GET /api/health HTTP/1.1\r\nHost: x\r\n" + b"".join(b"X-%d: v\r\n" % i for i in range(5000)) + b"\r\n")
    raw["cookie_100KB"] = raw_http(port, b"GET /api/me HTTP/1.1\r\nHost: x\r\nCookie: nf_session=" + b"a" * 100000 + b"\r\n\r\n")
    out["raw_protocol"] = raw
    out["total_jobs_in_db"] = cnt[0]
    return out


# ======================= LEVEL 4 =======================
OPS = [("calc", 30), ("get_calc", 14), ("list_calcs", 8), ("dashboard", 5), ("dxf", 10), ("projects", 5), ("new_project", 3), ("correct", 4), ("replay", 2), ("verify", 2), ("obligations", 4), ("health", 6), ("audit", 3), ("amoeba", 2), ("login", 2)]


def mixed_worker(i, port, cookie, pid, stop_at, rec, acked, rng_seed, username=None, think=0.0):
    rng = random.Random(rng_seed); c = Client(port, rec); c.cookie = cookie; mine = []; hdr = []
    names = [o for o, w in OPS for _ in range(w)]
    k = rng_seed * 100000
    while time.time() < stop_at and not STOP.is_set():
        op = rng.choice(names); k += 1
        if op == "calc" or not mine:
            fam = rng.choice(list(FAMILIES)); st, d = c.req("POST", f"/api/projects/{pid}/calculations", FAMILIES[fam](k), label="calc")
            if st == 200:
                mine.append((d["id"], fam)); acked.append(d["id"])
                if fam == "header_with_branch":
                    hdr.append(d["id"])
        elif op == "get_calc":
            c.req("GET", f"/api/calculations/{rng.choice(mine)[0]}", label="get_calc")
        elif op == "list_calcs":
            c.req("GET", "/api/calculations", label="list_calcs")
        elif op == "dashboard":
            c.req("GET", "/api/dashboard", label="dashboard")
        elif op == "dxf":
            cid, fam = rng.choice(mine); c.req("GET", f"/api/calculations/{cid}/dxf/{DXF_PART[fam]}", raw=True, label="dxf")
        elif op == "projects":
            c.req("GET", "/api/projects", label="projects")
        elif op == "new_project":
            c.req("POST", "/api/projects", {"name": f"w{i}-{k}"}, label="new_project")
        elif op == "correct" and hdr:
            cid = hdr.pop(rng.randrange(len(hdr)))
            st, d = c.req("POST", f"/api/calculations/{cid}/corrections", {"parameter": "header_radius", "new_value": 100 + rng.random() * 50, "reason": "mixed load correction"}, label="correct")
            if st == 200:
                hdr.append(d["new_calculation"]); acked.append(d["new_calculation"])
        elif op == "replay":
            c.req("POST", f"/api/calculations/{rng.choice(mine)[0]}/replay", label="replay")
        elif op == "verify":
            c.req("GET", "/api/audit/verify", label="audit_verify")
        elif op == "obligations":
            c.req("GET", "/api/obligations", label="obligations")
        elif op == "health":
            c.req("GET", "/api/health", label="health")
        elif op == "audit":
            c.req("GET", "/api/audit", label="audit_log")
        elif op == "amoeba":
            c.req("GET", "/api/amoeba/summary", label="amoeba")
        elif op == "login":
            cc = Client(port, rec); cc.login("admin", ADMIN[1], label="login")
        if think:
            time.sleep(think)


def level4(ctx):
    srv, rec, adm = ctx["srv"], ctx["rec"], ctx["admin"]
    port = srv.port; out = {"ramp": []}
    st, d = adm.req("POST", "/api/projects", {"name": "L4 concurrency"}); pid = d["id"]
    acked = []; per_phase = ctx.get("phase_s", 25)
    # seed
    for n in (1, 4, 16, 64):
        r2 = Recorder(); t0 = time.time(); stop_at = t0 + per_phase
        ths = [threading.Thread(target=mixed_worker, args=(i, port, adm.cookie, pid, stop_at, r2, acked, 1000 * n + i), daemon=True) for i in range(n)]
        [t.start() for t in ths]; [t.join() for t in ths]
        s = r2.summary(); s["users"] = n; s["by_op"] = r2.by_label(); s["rss_mb_end"] = srv.samples[-1]["rss_mb"] if srv.samples else None
        cpu = [x["cpu"] for x in srv.samples[-per_phase:]]; s["mean_cpu_pct"] = round(sum(cpu) / max(len(cpu), 1), 1)
        rec.rows.extend(r2.rows); rec.bad.extend(r2.bad); out["ramp"].append(s)
    # contention probe: health latency while N heavy ops are running
    cont = {}
    for kind in ("idle", "replay_x8", "login_x8", "create_calc_x8", "list_calcs_x4", "upload_20MB_x2"):
        stopev = threading.Event(); lat = []
        def bg(i, kind=kind):
            cc = Client(port); cc.cookie = adm.cookie
            while not stopev.is_set():
                if kind == "replay_x8": cc.req("POST", f"/api/calculations/{acked[i % len(acked)]}/replay")
                elif kind == "login_x8": Client(port).login()
                elif kind == "create_calc_x8": cc.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["header_with_branch"](random.random()))
                elif kind == "list_calcs_x4": cc.req("GET", "/api/calculations")
                elif kind == "upload_20MB_x2": cc.req("POST", f"/api/projects/{pid}/documents", b"%PDF-1.7\n" + os.urandom(20 * 1024 * 1024), headers={"X-Filename": "s.pdf"}, raw=True)
        nb = 0 if kind == "idle" else int(kind.split("_x")[1]); ths = [threading.Thread(target=bg, args=(i,), daemon=True) for i in range(nb)]; [t.start() for t in ths]
        time.sleep(1.0); cc = Client(port); cc.cookie = adm.cookie
        t_end = time.time() + 8
        while time.time() < t_end:
            t = time.time(); cc.req("GET", "/api/health"); lat.append((time.time() - t) * 1000); time.sleep(0.05)
        stopev.set(); [t.join() for t in ths]
        cont[kind] = {"health_p50_ms": round(pct(lat, 50), 1), "health_p95_ms": round(pct(lat, 95), 1), "health_max_ms": round(max(lat), 1), "n": len(lat)}
    out["contention_probe_health_latency"] = cont
    # (a) identical concurrent requests
    n0 = adm.req("GET", "/api/dashboard")[1]
    N = 64; body = FAMILIES["cone_frustum"](424242); res = []; lk = threading.Lock(); bar = threading.Barrier(N)
    def same(i):
        cc = Client(port, rec); cc.cookie = adm.cookie; bar.wait(); st, d = cc.req("POST", f"/api/projects/{pid}/calculations", body, label="race_identical")
        with lk: res.append((st, d))
    join(parallel(N, same)); ids = [d["id"] for st, d in res if st == 200]
    hashes = set(); 
    for cid in ids[:8]:
        hashes.add(adm.req("GET", f"/api/calculations/{cid}")[1]["closure_hash"])
    n1 = adm.req("GET", "/api/dashboard")[1]
    out["race_identical"] = {"sent": N, "ok": len(ids), "distinct_ids": len(set(ids)), "distinct_closure_hashes": len(hashes), "calc_count_delta": n1["calculations"] - n0["calculations"],
                             "records_delta": n1["records"] - n0["records"], "statuses": sorted(set(st for st, _ in res))}
    # (b) concurrent corrections on one job
    st, d = adm.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["header_with_branch"](3)); hid = d["id"]
    res = []; bar = threading.Barrier(32)
    def corr(i):
        cc = Client(port, rec); cc.cookie = adm.cookie; bar.wait()
        st, d = cc.req("POST", f"/api/calculations/{hid}/corrections", {"parameter": "header_radius", "new_value": 120 + i, "reason": f"race correction {i}"}, label="race_correct")
        with lk: res.append((st, d))
    join(parallel(32, corr)); okc = [d for st, d in res if st == 200]
    st, g = adm.req("GET", f"/api/calculations/{hid}")
    out["race_corrections_same_job"] = {"sent": 32, "ok_200": len(okc), "statuses": {str(s): sum(1 for x, _ in res if x == s) for s in set(x for x, _ in res)}, "superseded_by": g["superseded_by"]}
    # also corrections on same job chain: exactly one job should have supersedes==hid
    # (c) concurrent logins + password changes
    for i in range(4):
        new_engineer(adm, f"pw{i}", "initial-pass-12")
    res = []; bar = threading.Barrier(24)
    def pwrace(i):
        u = f"pw{i % 4}"; cc = Client(port, rec)
        if i < 16:
            bar.wait(); st = cc.login(u, "initial-pass-12", label="race_login")
            with lk: res.append(("login", u, st))
        else:
            cc.login(u, "initial-pass-12", label="race_login_pre"); bar.wait()
            st, d = cc.req("POST", "/api/password", {"current": "initial-pass-12", "new": f"changed-pass-{i}-x"}, label="race_pwchange")
            with lk: res.append(("pw", u, st, f"changed-pass-{i}-x"))
    join(parallel(24, pwrace))
    # exactly one change per user can succeed with 'current' = initial (others race: current check happens under lock -> after first, current is wrong => 403)
    summ = {}
    for u in {f"pw{i}" for i in range(4)}:
        wins = [r for r in res if r[0] == "pw" and r[1] == u and r[2] == 200]
        winner_pw = wins[0][3] if wins else None
        summ[u] = {"pw_change_200": len(wins), "new_pw_login": Client(port).login(u, winner_pw) if winner_pw else None, "old_pw_login": Client(port).login(u, "initial-pass-12")}
    out["race_login_password"] = {"login_statuses": sorted(set(r[2] for r in res if r[0] == "login")), "per_user": summ}
    # (d) audit chain after all
    v = adm.req("GET", "/api/audit/verify")[1]
    out["audit_verify_live"] = v
    out["acked_calcs"] = len(acked)
    ctx["acked"] = acked
    return out


def post_integrity(ctx, out):
    """Stop server cleanly, then verify files directly; restart for replay sample."""
    srv = ctx["srv"]
    srv.stop_sampler(); ok = srv.sigterm(10)
    ic = integrity_check(srv.data)
    srv.start(); c = Client(srv.port); c.login()
    ic["replay_sample"] = replay_sample(c, n=ctx.get("replay_n", 60))
    ic["suspect_replay"] = {"calcs": len(ic.get("suspect_owner_calcs", [])), "bad": sum(1 for cid in ic.get("suspect_owner_calcs", [])[:300] if not (lambda s_r: s_r[0] == 200 and s_r[1].get("matches") and not s_r[1].get("stored_records_failing_identity_check"))(c.req("POST", f"/api/calculations/{cid}/replay"))) }
    ic["suspect_ids"] = ic["suspect_ids"][:3]; ic.pop("suspect_owner_calcs", None)
    ic["audit_verify_api"] = c.req("GET", "/api/audit/verify")[1]
    ic["live_calc_listing_count"] = len(c.req("GET", "/api/calculations")[1])
    out["integrity"] = ic
    srv.stop()


# ======================= LEVEL 5 =======================
def level5(ctx):
    srv, rec, adm = ctx["srv"], ctx["rec"], ctx["admin"]
    port = srv.port; users = ctx.get("users", 4); think = ctx.get("think", 0.05)
    st, d = adm.req("POST", "/api/projects", {"name": "L5 endurance"}); pid = d["id"]; acked = []
    stop_at = time.time() + ctx["duration"]; t0 = time.time()
    ths = [threading.Thread(target=mixed_worker, args=(i, port, adm.cookie, pid, stop_at, rec, acked, 5000 + i, None, think), daemon=True) for i in range(users)]
    [t.start() for t in ths]
    while time.time() < stop_at:
        time.sleep(5)
        if not srv.alive():
            break
    [t.join() for t in ths]
    real = time.time() - t0
    s = [x for x in srv.samples if x["t"] > 30]  # ignore warm-up
    out = {"real_duration_s": round(real), "users": users, "think_s": think, "acked_calcs": len(acked), "server_alive": srv.alive()}
    for key in ("rss_mb", "fds", "threads", "data_mb"):
        fit = linfit([x["t"] for x in s], [x[key] for x in s])
        if fit:
            fit["per_hour"] = fit["slope"] * 3600
            half = len(s) // 2
            fit["first_half_mean"] = round(sum(x[key] for x in s[:half]) / max(half, 1), 2); fit["second_half_mean"] = round(sum(x[key] for x in s[half:]) / max(len(s) - half, 1), 2)
        out["fit_" + key] = fit
    # latency drift: p95 per quarter
    rows = sorted(rec.rows); q = max(len(rows) // 4, 1)
    out["latency_by_quarter"] = [{"p50_ms": round(pct([r[1] * 1000 for r in rows[i * q:(i + 1) * q]], 50), 2), "p95_ms": round(pct([r[1] * 1000 for r in rows[i * q:(i + 1) * q]], 95), 2)} for i in range(4)]
    out["note_6_24h"] = "NOT run. Command: python3 stress/run.py --level 5 --duration 86400 --users 4 --think 0.05 --port 8810 --bin <nf-server> --workdir <dir with >=60 GB free>  (data grows ~ MBs per minute; see report)"
    return out


# ======================= LEVEL 6 =======================
def writer_storm(port, cookie, pid, stop, acked, errs, nthreads=6, seed=0):
    def w(i):
        rng = random.Random(seed * 100 + i); c = Client(port, None, timeout=10); c.cookie = cookie; hdr = []; k = seed * 1000000 + i * 10000
        while not stop.is_set():
            k += 1; r = rng.random()
            if r < 0.5:
                fam = rng.choice(list(FAMILIES)); st, d = c.req("POST", f"/api/projects/{pid}/calculations", FAMILIES[fam](k))
                if st == 200:
                    acked.append(("calc", d["id"], fam))
                    if fam == "header_with_branch": hdr.append(d["id"])
            elif r < 0.7 and hdr:
                cid = hdr.pop()
                st, d = c.req("POST", f"/api/calculations/{cid}/corrections", {"parameter": "header_radius", "new_value": 100 + rng.random() * 50, "reason": "kill test correction"})
                if st == 200:
                    acked.append(("corr", d["new_calculation"], "header_with_branch")); hdr.append(d["new_calculation"])
            elif r < 0.8:
                st, d = c.req("POST", "/api/projects", {"name": f"k{k}"})
                if st == 200: acked.append(("proj", d["id"], None))
            elif r < 0.9:
                c.req("POST", f"/api/projects/{pid}/documents", b"%PDF-1.7\n" + os.urandom(200000), raw=True, headers={"X-Filename": "k.pdf"})
            else:
                c.req("GET", "/api/audit/verify")
    ths = [threading.Thread(target=w, args=(i,), daemon=True) for i in range(nthreads)]; [t.start() for t in ths]; return ths


def check_after_crash(srv, acked, pid_unused=None):
    """Return list of problems."""
    probs = []
    c = Client(srv.port, timeout=30); c.login()
    ic = integrity_check_live(srv.data)
    if ic["integrity_check"] != ["ok"]: probs.append(("integrity_check", ic["integrity_check"][:3]))
    if ic["audit_chain_python"]["broken_at"]: probs.append(("audit_chain", ic["audit_chain_python"]))
    st, v = c.req("GET", "/api/audit/verify")
    if st != 200 or not v.get("intact"): probs.append(("audit_verify_api", (st, v)))
    for kind, i, fam in acked:
        if kind in ("calc", "corr"):
            st, g = c.req("GET", f"/api/calculations/{i}")
            if st != 200: probs.append(("acked_job_missing", i)); continue
            if g.get("patterns") is None or not g.get("closure_hash"): probs.append(("job_incomplete", i))
            if g.get("superseded_by") is None:
                for p in g.get("patterns", []):
                    s2, b, _ = c.req("GET", f"/api/calculations/{i}/dxf/{p['part']}", raw=True)
                    if s2 != 200: probs.append(("dxf_unavailable_for_acked_job", (i, p["part"], s2)))
    return probs, ic


def integrity_check_live(data):
    """Read-only open while the server may be running (WAL)."""
    return integrity_check(data)


def level6_db(ctx, binary):
    base = ctx['workdir']; port = ctx['port']; out = {}
    # ---------- D. database damage (on copies)
    dbf = {}
    for mode in ("truncate_half", "truncate_0", "truncate_4096", "garbage_header", "delete_db_keep_blobs", "corrupt_middle_page", "delete_wal_midwrite"):
        dd = os.path.join(base, "dbdmg"); shutil.rmtree(dd, ignore_errors=True)
        s4 = Server(dd, port, binary, "d", data_dir=os.path.join(dd, "data")); assert s4.start(); a4 = fresh_admin(s4)
        st, d = a4.req("POST", "/api/projects", {"name": "dd"}); p4 = d["id"]
        for k in range(30): a4.req("POST", f"/api/projects/{p4}/calculations", FAMILIES["cone_frustum"](k))
        if mode == "delete_wal_midwrite":
            s4.kill9()
        else:
            s4.stop()
            if mode != 'garbage_wal_kept':
                import sqlite3 as _s
                _c = _s.connect(os.path.join(s4.data, 'nexus-fabric.sqlite')); _c.execute('PRAGMA wal_checkpoint(TRUNCATE)'); _c.close()
        f = os.path.join(s4.data, "nexus-fabric.sqlite"); sz = os.path.getsize(f)
        if mode == "delete_wal_midwrite":
            for e in ("-wal", "-shm"):
                try: os.remove(f + e)
                except OSError: pass
        elif mode == "truncate_half": os.truncate(f, sz // 2)
        elif mode == "truncate_0": os.truncate(f, 0)
        elif mode == "truncate_4096": os.truncate(f, 4096)
        elif mode == "garbage_header":
            with open(f, "r+b") as fh: fh.write(b"GARBAGE" * 20)
        elif mode == "delete_db_keep_blobs":
            os.remove(f)
        elif mode == "corrupt_middle_page":
            with open(f, "r+b") as fh: fh.seek(sz // 2); fh.write(os.urandom(4096))
        s5 = Server(dd, port, binary, "d", data_dir=s4.data); ok = s5.start(wait=6); r = {"starts": ok, "db_bytes": sz}
        if ok:
            c = Client(port); st, h = c.req("GET", "/api/health"); r["health"] = st; r["setup_required"] = (h or {}).get("setup_required")
            if r["setup_required"]:
                r["unauthenticated_POST_setup_status"] = Client(port).req("POST", "/api/setup", {"username": "attacker", "password": "attacker-pass-1"})[0]
                r["old_blobs_still_in_data_dir"] = len(os.listdir(os.path.join(s4.data, "blobs")))
            c.login(); 
            codes = {}
            for ep in ("/api/calculations", "/api/dashboard", "/api/audit/verify", "/api/projects", "/api/amoeba/summary", f"/api/calculations/5"):
                st, _ = c.req("GET", ep); codes[ep] = st
            r["endpoints"] = codes; r["audit_verify"] = c.req("GET", "/api/audit/verify")[1]
            r["new_calc"] = c.req("POST", f"/api/projects/{p4}/calculations", FAMILIES["cone_frustum"](999))[0]
            r["server_alive_after"] = s5.alive()
            r["log_tail"] = open(s5.log, "rb").read()[-200:].decode("latin1")
            s5.stop()
        else:
            r["exit_code"] = s5.proc.poll(); r["log_tail"] = open(s5.log, "rb").read()[-250:].decode("latin1")
        dbf[mode] = r
    out["db_damage"] = dbf
    return out['db_damage']


def level6(ctx):
    base = ctx["workdir"]; port = ctx["port"]; out = {}; binary = ctx["bin"]
    # ---------- A. kill -9 during writes
    reps = ctx.get("kill_reps", 25); kd = os.path.join(base, "kill"); shutil.rmtree(kd, ignore_errors=True)
    srv = Server(kd, port, binary, "kill"); assert srv.start(); adm = fresh_admin(srv)
    st, d = adm.req("POST", "/api/projects", {"name": "kill proj"}); pid = d["id"]
    for k in range(10):
        adm.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["cone_frustum"](k))
    acked_all = []; kill_log = []; rng = random.Random(7); problems_total = []; cookie = adm.cookie
    for rep in range(reps):
        stop = threading.Event(); acked = []; errs = []
        ths = writer_storm(port, cookie, pid, stop, acked, errs, 6, rep)
        time.sleep(rng.uniform(0.05, 1.5)); srv.kill9(); stop.set(); [t.join(timeout=15) for t in ths]
        acked_all += acked
        t = time.time(); started = srv.start(); startup = time.time() - t
        if not started:
            kill_log.append({"rep": rep, "start_failed": True}); problems_total.append(("restart_failed", rep)); break
        c = Client(port); c.login(); cookie = c.cookie
        probs, ic = check_after_crash(srv, acked[-60:])  # check recent acked jobs thoroughly
        kill_log.append({"rep": rep, "acked_in_run": len(acked), "restart_s": round(startup, 2), "problems": probs[:5]})
        problems_total += [(rep,) + p for p in probs]
    # final full check: all acknowledged ids
    probs, ic = check_after_crash(srv, acked_all[:600])
    # consistency: calcs with no audit entry / corrections without obligations / orphan records
    import sqlite3
    db = sqlite3.connect(f"file:{os.path.join(srv.data, 'nexus-fabric.sqlite')}?mode=ro", uri=True)
    no_audit = db.execute("SELECT COUNT(*) FROM calculations c WHERE c.supersedes IS NULL AND NOT EXISTS (SELECT 1 FROM audit a WHERE a.action='calculation' AND a.detail LIKE '#'||c.id||' %')").fetchone()[0]
    corr_no_obl = db.execute("SELECT COUNT(*) FROM calculations c WHERE c.supersedes IS NOT NULL AND NOT EXISTS (SELECT 1 FROM obligations o WHERE o.target_calc=c.id)").fetchone()[0]
    corr_no_audit = db.execute("SELECT COUNT(*) FROM calculations c WHERE c.supersedes IS NOT NULL AND NOT EXISTS (SELECT 1 FROM audit a WHERE a.action='correction' AND a.detail LIKE '%'||c.id||'%')").fetchone()[0]
    orphan_docs = db.execute("SELECT COUNT(*) FROM documents d WHERE NOT EXISTS (SELECT 1 FROM records r WHERE r.id=d.record_id)").fetchone()[0]
    doc_no_blob = sum(1 for (s,) in db.execute("SELECT sha256 FROM documents") if not os.path.exists(os.path.join(srv.data, "blobs", s + ".bin")))
    db.close()
    c = Client(port); c.login(); rs = replay_sample(c, 40)
    out["kill9"] = {"repetitions": len(kill_log), "acked_ops_total": len(acked_all), "problems": problems_total[:20], "n_problems": len(problems_total), "final_integrity": {k: v for k, v in ic.items() if k != "suspect_ids"},
                    "final_acked_check_problems": probs[:10], "calc_rows_without_audit_entry": no_audit, "correction_jobs_without_obligations": corr_no_obl, "correction_jobs_without_audit_entry": corr_no_audit,
                    "documents_without_record": orphan_docs, "documents_without_blob": doc_no_blob, "replay_sample": rs, "restart_s_max": max((k.get("restart_s", 0) for k in kill_log), default=0), "log": kill_log[:5]}
    # ---------- B. SIGTERM during requests
    stop = threading.Event(); acked = []; ths = writer_storm(port, c.cookie, pid, stop, acked, [], 6, 99)
    time.sleep(1.0); t = time.time(); srv.proc.send_signal(signal.SIGTERM)
    try:
        rc = srv.proc.wait(15); term_s = time.time() - t
    except subprocess.TimeoutExpired:
        rc = "HUNG"; term_s = 15; srv.kill9()
    stop.set(); [t.join(timeout=15) for t in ths]
    ok = srv.start(); c = Client(port); c.login(); probs, ic = check_after_crash(srv, acked[-60:])
    out["sigterm"] = {"exit_code": rc, "exit_s": round(term_s, 2), "restart_ok": ok, "problems": probs[:5], "integrity": ic["integrity_check"], "in_flight_acked": len(acked)}
    srv.stop()
    # ---------- C. key file faults
    kf = {}
    kdir = os.path.join(base, "keytest"); shutil.rmtree(kdir, ignore_errors=True); os.makedirs(kdir)
    keyfile = os.path.join(kdir, "storage.key")
    s2 = Server(kdir, port, binary, "key", key_file=keyfile); assert s2.start(); a2 = fresh_admin(s2)
    st, d = a2.req("POST", "/api/projects", {"name": "k"}); p2 = d["id"]
    st, d = a2.req("POST", f"/api/projects/{p2}/calculations", FAMILIES["cone_frustum"](1)); cid = d["id"]
    st, up, _ = a2.req("POST", f"/api/projects/{p2}/documents", b"%PDF-1.7\nhello secret", raw=True, headers={"X-Filename": "a.pdf"})
    s2.stop(); orig = open(keyfile, "rb").read()
    def restart_probe(label):
        s3 = Server(kdir, port, binary, "key", key_file=keyfile); ok = s3.start(wait=6); r = {"starts": ok}
        if ok:
            c = Client(port); c.login(); st, b, _ = c.req("GET", f"/api/calculations/{cid}/dxf/reducer", raw=True); r["dxf_status"] = st; r["dxf_msg"] = b[:100].decode("latin1")
            st, d = c.req("POST", f"/api/projects/{p2}/calculations", FAMILIES["cone_frustum"](2)); r["new_calc_status"] = st
            st, d, _ = c.req("POST", f"/api/projects/{p2}/documents", b"%PDF-1.7\nnew doc", raw=True, headers={"X-Filename": "n.pdf"}); r["new_upload_status"] = st
            r["audit_intact"] = c.req("GET", "/api/audit/verify")[1].get("intact")
            r["key_replaced_silently"] = os.path.exists(keyfile) and open(keyfile, "rb").read() != orig
            s3.stop()
        else:
            r["exit_code"] = s3.proc.poll(); r["log_tail"] = open(s3.log, "rb").read()[-300:].decode("latin1")
        kf[label] = r
    os.remove(keyfile); restart_probe("key_file_deleted")
    open(keyfile, "wb").write(b"short"); restart_probe("key_file_corrupt_5_bytes")
    open(keyfile, "wb").write(os.urandom(32)); restart_probe("key_file_replaced_with_other_32_bytes")
    open(keyfile, "wb").write(b""); restart_probe("key_file_empty")
    out["key_faults"] = kf
    out['db_damage'] = level6_db(ctx, binary)
    # ---------- E. write failure / disk full emulation
    out["disk_full"] = level6_disk(ctx, binary)
    shutil.rmtree(os.path.join(base, "dbdmg"), ignore_errors=True)
    return out


def consistency(data):
    import sqlite3
    db = sqlite3.connect(f"file:{os.path.join(data, 'nexus-fabric.sqlite')}?mode=ro", uri=True)
    q = lambda s: db.execute(s).fetchone()[0]
    o = {"calc_rows_without_audit_entry": q("SELECT COUNT(*) FROM calculations c WHERE c.supersedes IS NULL AND NOT EXISTS (SELECT 1 FROM audit a WHERE a.action='calculation' AND a.detail LIKE '#'||c.id||' %')"),
         "correction_jobs_without_obligations": q("SELECT COUNT(*) FROM calculations c WHERE c.supersedes IS NOT NULL AND NOT EXISTS (SELECT 1 FROM obligations o WHERE o.target_calc=c.id)"),
         "documents_without_blob": sum(1 for (s,) in db.execute("SELECT sha256 FROM documents") if not os.path.exists(os.path.join(data, "blobs", s + ".bin"))),
         "calcs_with_missing_dxf_blob": 0}
    for (rj,) in db.execute("SELECT result_json FROM calculations"):
        try:
            for p in json.loads(rj).get("patterns", []):
                s = p.get("dxf_sha256")
                if s and not os.path.exists(os.path.join(data, "blobs", s + ".bin")): o["calcs_with_missing_dxf_blob"] += 1
        except Exception: pass
    db.close(); return o


def level6_disk(ctx, binary):
    base = ctx["workdir"]; port = ctx["port"]; res = {}
    # try a real tmpfs / ro bind mount
    mnt = os.path.join(base, "tmpfs-mnt"); os.makedirs(mnt, exist_ok=True)
    r = subprocess.run(["mount", "-t", "tmpfs", "-o", "size=6m", "tmpfs", mnt], capture_output=True, text=True)
    real = r.returncode == 0
    res["real_small_filesystem"] = "tmpfs 6 MB mounted" if real else f"mount not permitted ({r.stderr.strip()[:80]}); using RLIMIT_FSIZE emulation instead"
    def run_case(label, datadir, limit_bytes=None, ro=False):
        shutil.rmtree(datadir, ignore_errors=True) if not real else None
        sv = Server(base, port, binary, "disk", data_dir=datadir)
        # preexec: ignore SIGXFSZ so EFBIG is returned instead of killing
        orig_popen = subprocess.Popen
        def pre():
            signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
            if limit_bytes is not None: resource.setrlimit(resource.RLIMIT_FSIZE, (limit_bytes, limit_bytes))
        import nfstress
        class P(orig_popen):
            def __init__(s, *a, **k): k["preexec_fn"] = pre; super().__init__(*a, **k)
        subprocess.Popen = P
        try: ok = sv.start()
        finally: subprocess.Popen = orig_popen
        r = {"starts": ok}
        if not ok:
            r["log_tail"] = open(sv.log, "rb").read()[-250:].decode("latin1"); return r
        a = fresh_admin(sv); st, d = a.req("POST", "/api/projects", {"name": "disk"}); p = d["id"] if d else 1
        statuses = {}; first_fail = None; n = 0; t_end = time.time() + 40
        while time.time() < t_end and n < 4000:
            n += 1; fam = ["cone_frustum", "header_with_branch"][n % 2]
            st, d = a.req("POST", f"/api/projects/{p}/calculations", FAMILIES[fam](n))
            statuses[st] = statuses.get(st, 0) + 1
            if st != 200 and first_fail is None: first_fail = n
            if st == 0 and not sv.alive(): break
            if st != 200 and statuses[st] > 40: break
        r["calc_status_counts"] = {str(k): v for k, v in statuses.items()}; r["first_failure_at_request"] = first_fail; r["server_alive"] = sv.alive()
        if sv.alive():
            r["reads_ok"] = [a.req("GET", ep)[0] for ep in ("/api/health", "/api/calculations", "/api/dashboard", "/api/audit/verify")]
            st, d = a.req("POST", "/api/projects", {"name": "after-full"}); r["create_project_after"] = st
            st, b, _ = a.req("POST", f"/api/projects/{p}/documents", b"%PDF-1.7\n" + os.urandom(2_000_000), raw=True, headers={"X-Filename": "x.pdf"}); r["upload_2MB"] = st
        r["log_tail"] = open(sv.log, "rb").read()[-300:].decode("latin1")
        sv.stop()
        # restart w/o limit and verify
        sv2 = Server(base, port, binary, "disk", data_dir=datadir); ok = sv2.start(); r["restart_ok"] = ok
        if ok:
            c = Client(port); c.login(); r["audit_intact_after_restart"] = c.req("GET", "/api/audit/verify")[1]
            sv2.stop()
            r["integrity_check"] = integrity_check(datadir)["integrity_check"]; r["consistency"] = consistency(datadir)
        return r
    if real:
        res["disk_full_6MB_tmpfs"] = run_case("tmpfs", mnt)
        subprocess.run(["umount", mnt], capture_output=True)
    else:
        res["write_limit_RLIMIT_FSIZE_3MB_file"] = run_case("fsize", os.path.join(base, "disk-fsize"), limit_bytes=3 * 1024 * 1024)
        # read-only data dir: root ignores chmod; try chattr +i
        d = os.path.join(base, "disk-ro"); shutil.rmtree(d, ignore_errors=True)
        sv = Server(base, port, binary, "ro", data_dir=d); sv.start(); a = fresh_admin(sv); sv.stop()
        ro = subprocess.run(["chattr", "-R", "+i", d], capture_output=True, text=True)
        if ro.returncode == 0:
            sv = Server(base, port, binary, "ro", data_dir=d); ok = sv.start(wait=6); r = {"starts": ok}
            if ok:
                c = Client(port); c.login(); r["login"] = c.cookie is not None; r["create_project"] = c.req("POST", "/api/projects", {"name": "ro"})[0]; r["server_alive"] = sv.alive(); sv.stop()
            else:
                r["log_tail"] = open(sv.log, "rb").read()[-250:].decode("latin1")
            subprocess.run(["chattr", "-R", "-i", d], capture_output=True); res["read_only_data_dir_chattr_i"] = r
        else:
            res["read_only_data_dir"] = f"SKIPPED: running as root (chmod ineffective) and chattr +i not permitted ({ro.stderr.strip()[:80]})"
        shutil.rmtree(d, ignore_errors=True)
    shutil.rmtree(os.path.join(base, "disk-fsize"), ignore_errors=True)
    return res


def level6b(ctx):
    """malformed HTTP + fuzz (run as part of level 6 via --level 6; separate instance)"""
    srv, rec, adm = ctx["srv"], ctx["rec"], ctx["admin"]; port = srv.port; out = {}
    st, d = adm.req("POST", "/api/projects", {"name": "fuzz"}); pid = d["id"]
    hx = lambda path, body=b"", extra=b"", ct=b"application/json", cookie=True: (b"POST " + path + b" HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\n" + (b"Cookie: " + adm.cookie.encode() + b"\r\n" if cookie else b"") +
                                                                             (b"Content-Type: " + ct + b"\r\n" if ct else b"") + extra + b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
    m = {}
    m["invalid_json"] = raw_http(port, hx(b"/api/projects", b"{not json"))
    m["empty_body_json"] = raw_http(port, hx(b"/api/projects", b""))
    m["wrong_content_type_text"] = raw_http(port, hx(b"/api/projects", b'{"name":"x"}', ct=b"text/plain"))
    m["no_content_type"] = raw_http(port, hx(b"/api/projects", b'{"name":"x"}', ct=None))
    m["invalid_utf8"] = raw_http(port, hx(b"/api/projects", b'{"name":"\xff\xfe"}'))
    m["content_length_mismatch_short"] = raw_http(port, b"POST /api/projects HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{\"name\":\"x\"}", timeout=6)
    m["cl_and_te_conflict"] = raw_http(port, b"POST /api/projects HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nContent-Type: application/json\r\nContent-Length: 12\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n")
    m["negative_content_length"] = raw_http(port, b"POST /api/projects HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nContent-Length: -1\r\n\r\n")
    m["http10"] = raw_http(port, b"GET /api/health HTTP/1.0\r\n\r\n")
    m["bad_method"] = raw_http(port, b"BREW /api/health HTTP/1.1\r\nHost: x\r\n\r\n")
    m["garbage_bytes"] = raw_http(port, os.urandom(2000) + b"\r\n\r\n")
    m["no_host"] = raw_http(port, b"GET /api/health HTTP/1.1\r\n\r\n")
    m["chunked_huge_chunk_size"] = raw_http(port, b"POST /api/projects HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\nFFFFFFFFFFFF\r\n", timeout=6)
    m["chunked_valid"] = raw_http(port, b"POST /api/projects HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nContent-Type: application/json\r\nCookie: " + adm.cookie.encode() + b"\r\nTransfer-Encoding: chunked\r\n\r\n10\r\n{\"name\":\"chunk\"}\r\n0\r\n\r\n")
    m["chunked_over_limit_300KB"] = raw_http(port, b"POST /api/projects HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nContent-Type: application/json\r\nCookie: " + adm.cookie.encode() + b"\r\nTransfer-Encoding: chunked\r\n\r\n" + hex(300000)[2:].encode() + b"\r\n" + b"a" * 300000 + b"\r\n0\r\n\r\n")
    m["origin_mismatch"] = raw_http(port, hx(b"/api/projects", b'{"name":"x"}', extra=b"Origin: http://evil\r\n"))
    out["malformed_http"] = m
    # health after the garbage
    out["alive_after_malformed"] = Client(port).req("GET", "/api/health")[0] == 200
    # slow body (slow-loris): hold N connections sending 1 byte every 2s for 20s
    def fds(): return psutil.Process(srv.proc.pid).num_fds()
    f0 = fds(); N = 300; socks = []
    for i in range(N):
        try:
            s = socket.create_connection(("127.0.0.1", port), timeout=5)
            s.sendall(b"POST /api/projects HTTP/1.1\r\nHost: x\r\nX-NF-Request: 1\r\nContent-Type: application/json\r\nCookie: " + adm.cookie.encode() + b"\r\nContent-Length: 1000\r\n\r\n{")
            socks.append(s)
        except OSError:
            break
    lat = []
    t_end = time.time() + 20; hc = Client(port)
    while time.time() < t_end:
        for s in socks[::10]:
            try: s.sendall(b" ")
            except OSError: pass
        t = time.time(); st, _ = hc.req("GET", "/api/health"); lat.append(((time.time() - t) * 1000, st)); time.sleep(0.5)
    f1 = fds(); still_open = 0
    for s in socks:
        try:
            s.settimeout(0.01); x = s.recv(1)
            if x == b"": pass
        except socket.timeout: still_open += 1
        except OSError: pass
    out["slowloris"] = {"held_connections": len(socks), "fds_before": f0, "fds_during": f1, "still_open_after_20s": still_open, "health_max_ms": round(max(l for l, _ in lat), 1), "health_statuses": sorted(set(s for _, s in lat)),
                        "note": "no server-side header/body read timeout observed if still_open ~= held"}
    for s in socks: s.close()
    time.sleep(1); out["fds_after_close"] = fds()
    # slow headers (never finish)
    socks = [socket.create_connection(("127.0.0.1", port)) for _ in range(100)]
    for s in socks: s.sendall(b"GET /api/health HTTP/1.1\r\nHost: x\r\nX-A: ")
    time.sleep(15); op = 0
    for s in socks:
        try:
            s.settimeout(0.01); x = s.recv(1)
        except socket.timeout: op += 1
        except OSError: pass
    out["slow_headers_100conn_still_open_after_15s"] = op
    for s in socks: s.close()
    # connection flood
    socks = []
    try:
        for i in range(5000):
            socks.append(socket.create_connection(("127.0.0.1", port), timeout=3))
    except OSError as e:
        out["flood_stopped_at"] = f"{len(socks)}: {e}"
    out["flood_connections_opened"] = len(socks); t = time.time(); st, _ = Client(port, timeout=10).req("GET", "/api/health"); out["health_during_flood"] = (st, round((time.time() - t) * 1000, 1))
    out["fds_during_flood"] = fds()
    for s in socks: s.close()
    time.sleep(1); out["alive_after_slow"] = Client(port).req("GET", "/api/health")[0] == 200
    # abort mid-body
    for i in range(50):
        s = socket.create_connection(("127.0.0.1", port)); s.sendall(hx(b"/api/projects", b'{"name":"x"}')[:-6]); s.close()
    # login lockout DoS: 40 bad logins/min from anyone block everybody
    lk = {}
    for i in range(41): Client(port).login("nobody%d" % i, "wrong-password-x")
    lk["valid_admin_login_after_41_bad_attempts_for_other_names"] = Client(port).login()
    lk["valid_admin_login_with_existing_session_still_works"] = adm.req("GET", "/api/me")[0]
    out["login_lockout"] = lk
    time.sleep(61)
    lk["valid_admin_login_after_61s"] = Client(port).login()
    # fuzz
    out["fuzz"] = fuzz(ctx, adm, pid)
    out["alive_end"] = srv.alive()
    return out


def mutate(rng, v, depth=0):
    r = rng.random()
    junk = [None, True, False, 0, -1, 1, 2 ** 63, -2 ** 63, 1e308, -1e308, 1e-320, 0.0, "", "a" * rng.choice([1, 100, 5000, 70000]), "\u0000", "\U0001F600" * 50, "' OR 1=1 --", "../../etc/passwd", [], {}, [1, 2, 3], {"a": {"b": None}}, "NaN", "1e999", 3.5, 1e16, 12345678901234567890]
    if isinstance(v, dict) and v and r < 0.6:
        v = dict(v); k = rng.choice(list(v))
        a = rng.random()
        if a < 0.2: del v[k]
        elif a < 0.5: v[k] = rng.choice(junk)
        elif a < 0.8: v[k] = mutate(rng, v[k], depth + 1)
        else: v[rng.choice(["x", "family", "id", k + "2", "__proto__", ""])] = rng.choice(junk)
        return v
    if isinstance(v, dict) and r < 0.7:
        return rng.choice(junk)
    if isinstance(v, (int, float)) and not isinstance(v, bool) and r < 0.9:
        return rng.choice([v * rng.choice([0, -1, 1e-9, 1e9, 1e100]), 0, -v, float("inf") if False else 1e308])
    if isinstance(v, str):
        return rng.choice([v + "z" * 1000, "", v[::-1], "\u0000" + v])
    return rng.choice(junk)


def fuzz(ctx, adm, pid):
    srv = ctx["srv"]; port = srv.port; rng = random.Random(1234); res = {}; N = ctx.get("fuzz_n", 500)
    a = Client(port); a.cookie = adm.cookie
    st, h = a.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["header_with_branch"](5)); hid = h["id"]
    st, up = a.req("POST", f"/api/projects/{pid}/documents", b"%PDF-1.7\nfz", headers={"X-Filename": "f.pdf"}); 
    new_engineer(adm, "fuzzeng", "fuzz-engineer-pass"); new_engineer(adm, "fuzzvw", "fuzz-viewer-pass", "viewer")
    # an obligation and advisory to poke
    st, cr = a.req("POST", f"/api/calculations/{hid}/corrections", {"parameter": "header_radius", "new_value": 141, "reason": "fuzz seed corr"})
    seeds = {
        "/api/login": {"username": "fuzzeng", "password": "fuzz-engineer-pass"},
        "/api/logout": {},
        "/api/users": {"username": "fzuser", "password": "fuzz-password-12", "role": "engineer"},
        "/api/users/{uid}/active": {"active": True},
        "/api/password": {"current": "fuzz-engineer-pass", "new": "fuzz-engineer-pass"},
        "/api/projects": {"name": "fz", "description": "fz"},
        "/api/projects/{pid}/members": {"user_id": 2},
        "/api/projects/{pid}/calculations": {"cylinder_shell": FAMILIES["cylinder_shell"](), "cone": FAMILIES["cone_frustum"](), "ecc": FAMILIES["eccentric_reducer"](), "stub": FAMILIES["branch_stub"](), "hdr": FAMILIES["header_with_branch"]()},
        "/api/calculations/{cid}/corrections": {"parameter": "header_radius", "new_value": 130, "reason": "fuzz reason"},
        "/api/advisories/1/acknowledge": {"reason": "fuzz ack reason"},
        "/api/obligations/1/resolve": {"decision": "confirmed", "reason": "fuzz resolve reason"},
        "/api/setup": {"username": "evil", "password": "evil-password-1"},
        "/api/calculations/{cid}/replay": {},
        "/api/projects/{pid}/documents": b"%PDF-1.7\n",
    }
    order = [k for k in seeds if k not in ("/api/logout", "/api/login", "/api/setup", "/api/password")] + ["/api/password", "/api/logout", "/api/login", "/api/setup"]
    fuzz_uids = [new_engineer(adm, f"fzt{i}", "fuzz-target-pass1")[1]["id"] for i in range(3)]
    for route in order:
        seed = seeds[route]; stats = {}; bad = []; n_route = N if route != "/api/password" else min(N, 120)
        for n in range(n_route):
            uid = rng.choice(fuzz_uids + [99999, -1, 0])
            path = route.replace("{pid}", str(pid)).replace("{cid}", str(hid)).replace("{uid}", str(uid))
            if route == "/api/projects/{pid}/calculations":
                body = mutate(rng, rng.choice(list(seed.values())))
            elif isinstance(seed, bytes):
                body = bytearray(seed + os.urandom(rng.choice([0, 5, 1000]))); body[rng.randrange(len(body))] ^= 0xFF; body = bytes(body)
            else:
                body = mutate(rng, seed)
            cl = Client(port, None, timeout=15)
            if route in ("/api/login", "/api/setup"):
                pass
            elif route == "/api/password":
                un = f"fzp{n}"; new_engineer(adm, un, "fuzz-engineer-pass"); cl.login(un, "fuzz-engineer-pass")
                if isinstance(body, dict) and rng.random() < 0.5 and "current" in body: body["current"] = "fuzz-engineer-pass"
            elif route == "/api/logout":
                cl.login("fuzzeng", "fuzz-engineer-pass") if n == 0 else None
                cl.cookie = adm.cookie if n == 0 else "nf_session=" + os.urandom(8).hex()
                if n == 0: cl.cookie = None
            else:
                cl.cookie = a.cookie
            st, r_, _ = cl.req("POST", path, body, raw=True, label="fuzz")
            stats[st] = stats.get(st, 0) + 1
            if st >= 500 or st == 0:
                bad.append({"path": path, "body": repr(body[:100]) if isinstance(body, bytes) else json.dumps(body)[:400], "status": st, "resp": r_[:150].decode("latin1")})
                if st == 0 and not srv.alive():
                    bad[-1]["crash"] = True; break
        res[route] = {"requests": sum(stats.values()), "status_counts": {str(k): v for k, v in sorted(stats.items())}, "5xx_or_conn_fail": len(bad), "examples": bad[:3]}
        if not srv.alive(): break
    return res


# ======================= driver =======================
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--level", type=int, required=True); ap.add_argument("--duration", type=int, default=60)
    ap.add_argument("--url", default=None); ap.add_argument("--port", type=int, default=8810)
    ap.add_argument("--bin", default=None); ap.add_argument("--workdir", default=None)
    ap.add_argument("--users", type=int, default=4); ap.add_argument("--think", type=float, default=0.05)
    ap.add_argument("--only", default=None, help="level 6: 'db' runs only the database damage scenarios")
    ap.add_argument("--quick", action="store_true", help="smaller sizes (smoke-test the framework)")
    ap.add_argument("--keep-data", action="store_true")
    a = ap.parse_args()
    port = urlparse(a.url).port if a.url else a.port
    if port is None or not (8810 <= port <= 8819):
        sys.exit("refusing: port must be in 8810-8819")
    wd = a.workdir or tempfile.mkdtemp(prefix="nfstress-"); os.makedirs(wd, exist_ok=True); os.makedirs(RESULTS, exist_ok=True)
    binary = a.bin or os.environ.get("NF_BIN") or os.path.join(os.path.dirname(HERE), "target", "release", "nf-server")
    ts = time.strftime("%Y%m%d-%H%M%S"); name = f"level{a.level}-{ts}"
    ctx = {"duration": a.duration, "workdir": wd, "port": port, "bin": binary, "users": a.users, "think": a.think}
    if a.quick:
        ctx.update(n_projects=60, n_jobs=60, stages=[100, 200], kill_reps=2, fuzz_n=20, phase_s=4, replay_n=5)
    t_start = time.time(); result = {"level": a.level, "started": ts, "machine": machine(), "binary": binary}
    if a.level == 6 and a.only == "db":
        result["db_damage"] = level6_db(ctx, binary); result["wall_s"] = 0
        with open(os.path.join(RESULTS, f"level6-dbdamage-{ts}.json"), "w") as f: json.dump(result, f, indent=1, default=str)
        print(json.dumps(result["db_damage"], indent=1, default=str)[:6000]); return
    if a.level == 6:
        out = level6(ctx)  # manages own servers
        result["fault_injection"] = out
        srv = Server(wd, port, binary, "fuzz"); shutil.rmtree(srv.data, ignore_errors=True); srv.start(); rec = Recorder(); srv.start_sampler(); adm = fresh_admin(srv)
        ctx.update(srv=srv, rec=rec, admin=adm)
        result["malformed_and_fuzz"] = level6b(ctx); srv.stop(); result["server"] = srv.sample_summary()
        result["http"] = rec.summary(); result["bad_requests"] = rec.bad[:20]
    else:
        srv = Server(wd, port, binary, f"l{a.level}"); shutil.rmtree(srv.data, ignore_errors=True)
        assert srv.start(), "server failed to start"; rec = Recorder(); adm = fresh_admin(srv); adm.rec = None
        ctx.update(srv=srv, rec=rec, admin=adm); srv.start_sampler()
        fn = {1: level1, 2: level2, 3: level3, 4: level4, 5: level5}[a.level]
        t0 = time.time()
        try:
            out = fn(ctx)
        finally:
            pass
        wall = time.time() - t0
        result["details"] = out; result["http"] = rec.summary(dur=wall); result["by_label"] = rec.by_label(); result["bad_requests"] = rec.bad[:30]
        result["server"] = srv.sample_summary(); result["samples"] = srv.samples[::max(1, len(srv.samples) // 600)]
        result["server_alive_at_end"] = srv.alive()
        if a.level in (4, 2, 3):
            post_integrity(ctx, result["details"])
        srv.stop()
        if not a.keep_data:
            shutil.rmtree(srv.data, ignore_errors=True)
    result["wall_s"] = round(time.time() - t_start, 1)
    with open(os.path.join(RESULTS, name + ".json"), "w") as f: json.dump(result, f, indent=1, default=str)
    with open(os.path.join(RESULTS, name + ".md"), "w") as f: f.write(markdown(result))
    print(markdown(result)[:3000]); print("results:", os.path.join(RESULTS, name + ".json"))


def markdown(r):
    L = [f"# Level {r['level']} ({r['started']})", "", f"machine: {r['machine']}", f"wall: {r['wall_s']} s", ""]
    h = r.get("http")
    if h:
        L += ["## HTTP", "", "| requests | 5xx/conn errors | p50 ms | p95 ms | p99 ms | max ms | rps |", "|---|---|---|---|---|---|---|",
              f"| {h.get('requests')} | {h.get('errors_5xx_or_conn')} | {h.get('p50_ms')} | {h.get('p95_ms')} | {h.get('p99_ms')} | {h.get('max_ms')} | {h.get('throughput_rps')} |", ""]
    if r.get("server"): L += ["## Server process", "", "```", json.dumps(r["server"]), "```", ""]
    if r.get("by_label"):
        L += ["## Per operation", "", "| op | n | p50 | p95 | p99 | max | rps |", "|---|---|---|---|---|---|---|"]
        for k, v in r["by_label"].items(): L.append(f"| {k} | {v['requests']} | {v['p50_ms']} | {v['p95_ms']} | {v['p99_ms']} | {v['max_ms']} | {v['throughput_rps']} |")
        L.append("")
    body = {k: v for k, v in r.items() if k in ("details", "fault_injection", "malformed_and_fuzz", "bad_requests")}
    L += ["## Details (JSON)", "", "```json", json.dumps(body, indent=1, default=str)[:60000], "```"]
    return "\n".join(L)


if __name__ == "__main__":
    main()
