#!/usr/bin/env python3
# Writes docs/release/manifest-<commit>.json and MANIFEST-<commit>.md from the logs of packaging/verify_release.sh,
# and copies both into the output folder. Reads only the verifier's own logs and the artefacts in the output folder.
# Development build. Not validated. Outputs must not be used for manufacturing.
import argparse
import datetime
import hashlib
import json
import os
import shutil
import sys


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def tsv(path, cols):
    rows = []
    if not os.path.exists(path):
        return rows
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line:
                continue
            parts = line.split("\t")
            parts += [""] * (len(cols) - len(parts))
            rows.append(dict(zip(cols, parts[: len(cols)])))
    return rows


NOT_SHOWN = [
    "The Windows checks ran under wine 9.x, an emulator on Linux. They were NOT run on Windows. No result here is a Windows result.",
    "The packages and the programme are NOT code-signed. Windows will warn about unknown publishers and SmartScreen will apply.",
    "No macOS package exists. macOS was not built, installed or run.",
    "There is no code-signing, no notarisation and no publisher identity in any artefact.",
    "The upgrade path was exercised only if an earlier installer was given with --previous; otherwise it is recorded as not run.",
    "The GitHub release workflow (.github/workflows/release.yml) was not run or exercised; this verification is local only.",
    "No release was created, tagged, published or uploaded. Nothing was pushed.",
    "The browser checks cover the first-run page of desktop mode; they do not test the engineering calculations.",
    "Nothing here is a validation of the engineering results. Development build. Not validated. Outputs must not be used for manufacturing.",
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--docs", required=True)
    ap.add_argument("--short", required=True)
    a = ap.parse_args()
    logs = os.path.join(a.out, "logs")

    facts_rows = tsv(os.path.join(logs, "facts.tsv"), ["key", "value"])
    facts = {r["key"]: r["value"] for r in facts_rows}
    results = tsv(os.path.join(logs, "results.tsv"), ["id", "title", "how", "status", "reason", "evidence", "log"])
    versions = tsv(os.path.join(logs, "versions.tsv"), ["label", "value", "how", "agrees", "note"])
    build_steps = [r for r in results if r["id"].startswith("build.")]

    deps = []
    dep_path = os.path.join(logs, "deps.txt")
    if os.path.exists(dep_path):
        with open(dep_path, encoding="utf-8", errors="replace") as f:
            deps = [l.strip() for l in f if l.strip()]

    artefacts = []
    for name in sorted(os.listdir(a.out)):
        if name.startswith("nexus-fabric-") and os.path.isfile(os.path.join(a.out, name)):
            p = os.path.join(a.out, name)
            artefacts.append({"file": name, "bytes": os.path.getsize(p), "sha256": sha256(p)})
    sums = os.path.join(a.out, "SHA256SUMS")
    sums_sha = sha256(sums) if os.path.exists(sums) else "missing"

    # Platform summary: what ran where.
    def how_count(prefix):
        rs = [r for r in results if r["how"].startswith(prefix) and not r["id"].startswith("build.")]
        return {s: sum(1 for r in rs if r["status"] == s) for s in ("passed", "failed", "not run")}, len(rs)

    lin_counts, lin_n = how_count("native Linux")
    win_counts, win_n = how_count("emulated")
    platform = [
        {"platform": "Linux", "how": "native (run on this Linux host)", "checks": lin_n, **lin_counts},
        {"platform": "Windows", "how": "emulator only (wine " + facts.get("wine", "?") + "). Not run on Windows.", "checks": win_n, **win_counts},
        {"platform": "macOS", "how": "no package exists", "checks": 0, "passed": 0, "failed": 0, "not run": 0},
    ]

    # Reproducibility: the packages of this run against the second build of the same commit (logs/repro.tsv).
    repro = {"status": facts.get("reproducibility", "not recorded"), "bit_reproducible": None, "rows": []}
    for r in tsv(os.path.join(logs, "repro.tsv"), ["file", "this_run", "second_build"]):
        repro["rows"].append({"file": r["file"], "this_run": r["this_run"], "second_build": r["second_build"],
                              "same": bool(r["this_run"]) and r["this_run"] == r["second_build"]})
    if repro["rows"]:
        repro["bit_reproducible"] = len(repro["rows"]) == 3 and all(x["same"] for x in repro["rows"])

    versions_ok = all(v["agrees"] == "yes" for v in versions) and bool(versions)
    failed = [r for r in results if r["status"] == "failed"]
    not_run = [r for r in results if r["status"] == "not run"]
    build_ok = all(r["status"] == "passed" for r in build_steps) and bool(build_steps)

    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    manifest = {
        "title": "NEXUS FABRIC release verification manifest",
        "disclaimer": "Development build. Not validated. Outputs must not be used for manufacturing.",
        "generated_utc": now,
        "source": {
            "commit": facts.get("commit"),
            "short_commit": facts.get("short_commit"),
            "commit_date": facts.get("commit_date"),
            "subject": facts.get("subject"),
            "tree_sha": facts.get("tree_sha"),
            "version": facts.get("version"),
            "built_from": "git archive of the commit (a fresh copy); the working tree is not used",
        },
        "tree_state": {
            "core_paths_crates_web_packaging": facts.get("core_tree"),
            "other_changes_at_run_time": [x for x in facts.get("other_changes", "").split(";") if x],
        },
        "build_environment": {
            "os": facts.get("os"), "kernel": facts.get("kernel"), "host_triple": facts.get("rustc_host"),
            "target_triples": facts.get("target_triples"), "installed_rust_targets": facts.get("rustup_targets"),
            "cargo_target_cache": facts.get("cargo_target_cache"),
        },
        "tools": {k: facts.get(k) for k in ("rustc", "cargo", "mingw_gcc", "makensis", "zip", "wine", "git", "python", "node", "playwright", "curl", "unshare")},
        "dependencies": {
            "cargo_lock_sha256": facts.get("cargo_lock_sha256"),
            "server_crate_resolved_count": len(deps),
            "server_crate_resolved": deps,
        },
        "build_steps": [{"step": r["id"], "result": r["status"], "evidence": r["evidence"]} for r in build_steps],
        "artefacts": artefacts,
        "sha256sums_file_sha256": sums_sha,
        "previous_installer": {"file": facts.get("previous_installer"), "sha256": facts.get("previous_installer_sha256")},
        "version_agreement": {"all_agree": versions_ok, "rows": versions},
        "checks": [{"id": r["id"], "title": r["title"], "how_it_ran": r["how"], "result": r["status"],
                    "reason": r["reason"], "evidence": r["evidence"], "log": r["log"]} for r in results if not r["id"].startswith("build.")],
        "platform_summary": platform,
        "summary": {"build_ok": build_ok, "checks_passed": sum(1 for r in results if r["status"] == "passed"),
                    "checks_failed": len(failed), "checks_not_run": len(not_run)},
        "reproducibility": repro,
        "not_shown": NOT_SHOWN,
    }
    if repro["bit_reproducible"] is False:
        manifest["not_shown"] = NOT_SHOWN + [
            "The build is NOT bit-reproducible: the two builds of the same commit produced different SHA-256 values. Both sets are recorded in the reproducibility section."
        ]

    short = a.short
    jpath = os.path.join(a.docs, f"manifest-{short}.json")
    mpath = os.path.join(a.docs, f"MANIFEST-{short}.md")
    os.makedirs(a.docs, exist_ok=True)
    with open(jpath, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
        f.write("\n")
    with open(mpath, "w", encoding="utf-8") as f:
        f.write(markdown(manifest))
    for p in (jpath, mpath):
        shutil.copy(p, a.out)
    print(f"wrote {jpath}")
    print(f"wrote {mpath}")
    return 0


def cell(s):
    return str(s if s not in (None, "") else "-").replace("|", "\\|").replace("\n", " ")


def markdown(m):
    s = m["source"]
    o = []
    o.append(f"# NEXUS FABRIC release verification: {s['short_commit']}")
    o.append("")
    o.append("> " + m["disclaimer"])
    o.append("")
    o.append(f"- Source commit: `{s['commit']}`")
    o.append(f"- Commit date: {s['commit_date']}")
    o.append(f"- Subject: {s['subject']}")
    o.append(f"- Tree SHA: `{s['tree_sha']}`")
    o.append(f"- Version (Cargo.toml): {s['version']}")
    o.append(f"- Report generated (UTC, the only time-dependent field): {m['generated_utc']}")
    o.append("")
    o.append("## Tree state")
    o.append("")
    o.append(f"- {m['tree_state']['core_paths_crates_web_packaging']}")
    other = m["tree_state"]["other_changes_at_run_time"]
    o.append("- Other changes outside those paths (reported, not fatal): " + ("; ".join(other) if other else "none"))
    o.append("- The build used a `git archive` of the commit, not the working tree.")
    o.append("")
    o.append("## Summary by platform")
    o.append("")
    o.append("| Platform | How it was checked | Checks | Passed | Failed | Not run |")
    o.append("|---|---|---|---|---|---|")
    for p in m["platform_summary"]:
        o.append(f"| {p['platform']} | {cell(p['how'])} | {p['checks']} | {p['passed']} | {p['failed']} | {p['not run']} |")
    o.append("")
    o.append("Linux = native. Windows = emulator only (wine), not Windows. macOS = no package exists.")
    o.append("")
    o.append(f"Build: {'all steps passed' if m['summary']['build_ok'] else 'NOT all steps passed'}. "
             f"Checks: {m['summary']['checks_passed']} passed, {m['summary']['checks_failed']} failed, {m['summary']['checks_not_run']} not run.")
    o.append("")
    o.append("## Build environment and tools")
    o.append("")
    for k, v in m["build_environment"].items():
        o.append(f"- {k}: {cell(v)}")
    o.append("")
    o.append("| Tool | Version |")
    o.append("|---|---|")
    for k, v in m["tools"].items():
        o.append(f"| {k} | {cell(v)} |")
    o.append("")
    o.append("## Build steps (exit status)")
    o.append("")
    o.append("| Step | Result | Evidence |")
    o.append("|---|---|---|")
    for b in m["build_steps"]:
        o.append(f"| {b['step']} | {b['result']} | {cell(b['evidence'])} |")
    o.append("")
    o.append("## Artefacts")
    o.append("")
    o.append("| File | Bytes | SHA-256 |")
    o.append("|---|---|---|")
    for x in m["artefacts"]:
        o.append(f"| {x['file']} | {x['bytes']} | `{x['sha256']}` |")
    o.append("")
    o.append(f"SHA256SUMS file: `{m['sha256sums_file_sha256']}`")
    o.append("")
    o.append("## Dependencies")
    o.append("")
    o.append(f"- Cargo.lock SHA-256: `{m['dependencies']['cargo_lock_sha256']}`")
    o.append(f"- Resolved dependencies of the server crate (`cargo tree --locked -p nf-server -e normal`): {m['dependencies']['server_crate_resolved_count']} entries; the full list is in the JSON manifest.")
    o.append("")
    o.append("## Version agreement")
    o.append("")
    o.append(f"All rows agree: {'yes' if m['version_agreement']['all_agree'] else 'NO'}")
    o.append("")
    o.append("| Item | Value | How it ran | Agrees with Cargo.toml |")
    o.append("|---|---|---|---|")
    for r in m["version_agreement"]["rows"]:
        o.append(f"| {cell(r['label'])} | {cell(r['value'])} | {cell(r['how'])} | {r['agrees']} |")
    o.append("")
    o.append("## Install, upgrade, offline first launch, uninstall and other checks")
    o.append("")
    o.append("Each result says how it ran. A check that did not run says why. Logs are in the output folder under `logs/`.")
    o.append("")
    o.append("| Check | How it ran | Result | Reason (if not passed) | One-line evidence | Log |")
    o.append("|---|---|---|---|---|---|")
    for c in m["checks"]:
        o.append(f"| `{c['id']}` {cell(c['title'])} | {cell(c['how_it_ran'])} | **{c['result']}** | {cell(c['reason'])} | {cell(c['evidence'])} | {cell(c['log'])} |")
    o.append("")
    r = m["reproducibility"]
    o.append("## Reproducibility")
    o.append("")
    o.append(f"Second build: {r['status']}.")
    if r["rows"]:
        o.append("")
        o.append(f"Bit-for-bit identical in the two builds: **{'yes' if r['bit_reproducible'] else 'NO'}**")
        o.append("")
        o.append("| File | SHA-256, build 1 (artefacts in this folder) | SHA-256, build 2 | Same |")
        o.append("|---|---|---|---|")
        for d in r["rows"]:
            o.append(f"| {d['file']} | `{d['this_run']}` | `{d['second_build']}` | {'yes' if d['same'] else 'no'} |")
        if not r["bit_reproducible"]:
            o.append("")
            o.append("The builds are NOT bit-identical. Both sets of SHA-256 values are shown above.")
    o.append("")
    o.append("## What this does NOT show")
    o.append("")
    for line in m["not_shown"]:
        o.append(f"- {line}")
    o.append("")
    o.append("Development build. Not validated. Outputs must not be used for manufacturing.")
    o.append("")
    return "\n".join(o)


if __name__ == "__main__":
    sys.exit(main())
