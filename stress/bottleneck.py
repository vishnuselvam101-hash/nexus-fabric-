#!/usr/bin/env python3
"""Evidence-gathering for the performance bottleneck (no optimisation, measurement only).
  python3 stress/bottleneck.py --port 8816 --bin <nf-server> --workdir <dir>
Measures: throughput scaling 1/4/16 clients for write / read / health / login, same workloads with the data dir on
tmpfs (/dev/shm, removes disk sync cost), per-thread CPU of the server under 16 writers, and fsync counts per request (strace)."""
import argparse, json, os, shutil, signal, subprocess, sys, threading, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from nfstress import *


def drive(port, cookie, n, secs, fn):
    cnt = [0]; lats = []; lk = threading.Lock(); end = time.time() + secs
    def w(i):
        c = Client(port); c.cookie = cookie; k = i * 1000000
        while time.time() < end:
            k += 1; t = time.time(); fn(c, k); d = time.time() - t
            with lk: cnt[0] += 1; lats.append(d * 1000)
    ths = [threading.Thread(target=w, args=(i,)) for i in range(n)]; t0 = time.time(); [t.start() for t in ths]; [t.join() for t in ths]
    return {"clients": n, "rps": round(cnt[0] / (time.time() - t0), 1), "p50_ms": round(pct(lats, 50), 2), "p95_ms": round(pct(lats, 95), 2)}


def run_set(srv, label, out):
    a = fresh_admin(srv); st, d = a.req("POST", "/api/projects", {"name": "b"}); pid = d["id"]
    ids = [a.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["cone_frustum"](i))[1]["id"] for i in range(100)]
    ops = {"write_calc_cone": lambda c, k: c.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["cone_frustum"](k)),
           "read_get_calc": lambda c, k: c.req("GET", f"/api/calculations/{ids[k % 100]}"),
           "read_health": lambda c, k: c.req("GET", "/api/health"),
           "login_argon2": lambda c, k: Client(srv.port).login()}
    res = {}
    for op, fn in ops.items():
        res[op] = [drive(srv.port, a.cookie, n, 6, fn) for n in (1, 4, 16)]
    out[label] = res


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--port", type=int, default=8816); ap.add_argument("--bin"); ap.add_argument("--workdir", required=True); a = ap.parse_args()
    assert 8810 <= a.port <= 8819
    out = {"machine": machine()}
    for label, base in (("data_on_disk", a.workdir), ("data_on_tmpfs_devshm", "/dev/shm/nfstress-bn")):
        shutil.rmtree(base, ignore_errors=True)
        srv = Server(base, a.port, a.bin, "bn"); assert srv.start(); run_set(srv, label, out)
        if label == "data_on_disk":
            # per-thread cpu under 16 writers, and strace fsync counts
            c = fresh_admin(srv) if False else None
            cl = Client(srv.port); cl.login(); pid = cl.req("POST", "/api/projects", {"name": "t"})[1]["id"]
            ps = srv.ps; before = {t.id: t.user_time + t.system_time for t in ps.threads()}
            r = drive(srv.port, cl.cookie, 16, 8, lambda c, k: c.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["header_with_branch"](k)))
            after = {t.id: t.user_time + t.system_time for t in ps.threads()}
            d = sorted(((after[k] - before.get(k, 0)) for k in after), reverse=True)
            out["per_thread_cpu_seconds_16_writers_8s"] = {"throughput": r, "thread_cpu_s_desc": [round(x, 2) for x in d], "total_cpu_s": round(sum(d), 2), "wall_s": 8}
            # strace: syscalls per calc (sequential)
            sp = subprocess.Popen(["strace", "-f", "-c", "-e", "trace=fsync,fdatasync,pwrite64,write,pread64,futex,openat,fcntl", "-p", str(srv.proc.pid), "-o", os.path.join(a.workdir, "strace.txt")], stderr=subprocess.DEVNULL)
            time.sleep(1.0); N = 50
            t = time.time()
            for i in range(N): cl.req("POST", f"/api/projects/{pid}/calculations", FAMILIES["header_with_branch"](i + 5000))
            dt = time.time() - t
            sp.send_signal(signal.SIGINT); sp.wait(); txt = open(os.path.join(a.workdir, "strace.txt")).read()
            out["strace_50_sequential_header_calcs_(slowed_by_strace)"] = {"seconds": round(dt, 2), "table": txt[-1500:], "calcs": N}
        srv.stop(); shutil.rmtree(base, ignore_errors=True)
    json.dump(out, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "results", "bottleneck.json"), "w"), indent=1)
    print(json.dumps(out, indent=1)[:6000])


if __name__ == "__main__":
    main()
