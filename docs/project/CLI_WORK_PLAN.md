# NEXUS FABRIC — full work plan for the CLI

Written 21 Nov 2026 by the cloud session's lead. State it starts from: branch
`claude/nexus-fabric-strategy-8guhxz`, engine 0.4.2 (commit `ed4a5fb`), docs commit `ccaa7f2`.
Read `docs/project/NEXUS_FABRIC_COMPLETE_HANDOFF.md` first; this plan replaces its section 10.

Save this file in the repository as `docs/project/CLI_WORK_PLAN.md` and keep the status column of each task current.

**Development build. Not validated. Outputs must not be used for manufacturing.**

---

## Status of every task (kept current by the CLI)

Statuses: `not started` · `in progress` · `done` (with the commit that holds it) · `blocked` (with what it waits
for) · `design — awaiting owner` · `not on this host` · `tracked` (people only). "Done" means the work was run and
seen to pass on the stated commit; "not run" is never written as passed.

| Task | Subject | Status | Commit | Note |
|---|---|---|---|---|
| 0.1 | Working on the live server made safe | done | `36072a1` | facts under the task; `nf-sandbox` + `nf-verify` |
| 0.2 | Readiness and registers tell the truth | done | this commit | READINESS geometry row, RB-011, B2-001…007 rows, fabrication questions (10), hand-off section 6 |
| 0.3 | The two near-tangent FAILs (D4) | done (facts) | this commit | remedy folded into 1.2 (decision under the task); regression cases land with 1.2 |
| 1.1 | Shallow and near-180° branch angles | done | `1ec0f97` | engine 0.4.3; code `branch_angle_not_verifiable`; decision and derivation under the task, TOLERANCE_POLICY section 7 |
| 1.2 | A true error bound everywhere | done | 2100a22 (+ uncommitted expectation fix-up) | engine 0.4.4; certified eps_rep (TOLERANCE_POLICY section 8); near-tangent FAILs resolved; changed expectations in section 8.8 and the fix-up table after it; fix-up: PRODUCT_CHECK_INCONCLUSIVE split, ui/e2e expectations, eps_num/eps_rep allow-list |
| 1.3 | Every convention stated in the DXF and the result | done | `3874a79` | engine 0.4.5; 12 plate-convention keys (999 + TEXT) per family, read back by rule `convention_statement` (14 rules); RB-008 outer profile one closed polyline where the arcs allow (lines/polylines families); changed expectations and i18n keys under the task (section 1.3 result) |
| 1.4 | Surface attribute for every family | done | `06f10d9` | engine 0.4.6; optional `surface_reference` (outside/inside/neutral/unknown) for cylinder shell, cone, eccentric reducer; `unknown` demotes radius checks to INCONCLUSIVE with a stated reason; geometry byte-identical; results in TOLERANCE_POLICY section 9 |
| 1.5 | Checks that do not depend on the construction | done | `055c846` | engine 0.4.8; RC-09/10/13/15/16/29/32 fixed, verifier independence guarded (bit-equal re-derived bounds), DXF-only mating test in validation stage 0 (sensitivity-bound tolerance, six planted faults FAIL); results under the task and TOLERANCE_POLICY sections 11–13 |
| 1.6 | Sampling density | not started | | |
| 1.7 | Set-on joint, remaining case — design note | done (design) | `2bf369a` | design only |
| 1.8 | Validation as a permanent gate | not started | | |
| 1.9 | Third independent review | not started | | after 1.1–1.6 |
| 2.1 | The five failing audit checks | done | this commit | security_r6 40/40; V-45 + V-44 here, V-47 from `95927a7` (RC-08); installer test `tests/install_check.sh` 67/67 (strict sandbox only); Caddy repository key still fetched without a fingerprint check (V-44 open item) |
| 2.2 | Architecture review findings RA-001 … RA-017 | not started | | |
| 2.3 | Stress findings | not on this host | | needs a non-live machine |
| 2.4 | Desktop security wording | done | this commit | `/api/health` says `mode` (desktop/server); first-run screen in desktop mode (EN + JA), `packaging/INSTALL.md`, `packaging/README.txt`; "secure backups" written as "protected backups" |
| 2.5 | Database encryption — design | done (design) | `2bf369a` | design only |
| 2.6 | Sign-in hardening — design note | done (design) | `2bf369a` | design only |
| 3.1 | PDF → DXF inside the product | done | `a12dec4` | reviewed (2 rounds + installer delta). Open: converter not in the downloadable packages; Windows not run; prebuilt `NF_BINARY` install does not install/verify `nf-pdf2dxf` (prints a note) — add `NF_PDF2DXF_BINARY` + `NF_PDF2DXF_SHA256` (task 4.2); rollback-restores-converter test |
| 3.2 | Forms and refusals | not started | | |
| 3.3 | Words and language | not started | | |
| 3.4 | Speed and scale | not started | | |
| 3.5 | Interface quality | not started | | |
| 4.1 | Reproducible builds | not started | | |
| 4.2 | Release verification | not started | | Windows part needs a real Windows PC |
| 4.3 | Continuous integration | not started | | |
| 4.4 | Release-candidate report | not started | | |
| 4.5 | Legal drafts | done | `011b061` | |
| 4.6 | The live server | not started | | only when the owner says so |
| RC | External review C (RC-01 … RC-33): verify every finding, fix what is real | in progress | | register: `docs/backend/REVIEW_C_EXTERNAL.md`; fixes tracked to the tasks named there; RC-01/RC-08/RC-22 fixed |
| 5.x | Later work | not started | | the owner asked on 21 Nov 2026 for the complete software: taken up after phase 4 |
| 6.x | What only people can supply | tracked | | |

**Owner decision, 21 Nov 2026 (in the CLI session).** Asked how far the CLI may decide on its own, the owner answered:
"like you do which is best at last i need this complete software". The CLI therefore takes the decisions this plan
reserves for the owner — the shallow-angle rule of 1.1 (the plan's proposal), engine-version bumps (one per task that
changes results, with older results marked for recomputation by G21), and implementation of the design documents
after an adversarial review — and reports each one. Not covered, still only on the owner's explicit word: anything on
the live server (4.6), pull requests, tags and releases (rule 10), and any value only a fabrication engineer can
give (rule 4).

---

## A. Rules for every task (never broken)

1. Correctness first. Fail closed: refuse or report INCONCLUSIVE; never guess.
2. Never weaken, skip, delete or loosen a test or a tolerance. Every changed expectation is listed with old value,
   new value and reason.
3. Nothing reaches `APPROVED`. Every DXF stays NOT RELEASED. Never write approved, certified, validated,
   production-ready, secure, hack-proof, error-free or AI-native.
4. Invent no number: no dimension, process value, clearance, K-factor, tolerance from a standard, benchmark or
   accuracy figure. If a rule needs a value nobody has, record it as a question for a fabrication engineer.
5. Thickness, bend allowance, weld gap and kerf are not modelled. Say nothing that suggests otherwise.
6. A new parser runs isolated, without network, with resource limits, and has fuzz tests.
7. Records are append-only. No AI in the product. Fully offline: no network call, telemetry, CDN or auto-update.
8. Only one change to the pattern mathematics at a time; cut geometry stays byte-identical unless the task says it
   changes, and then the engine version is bumped and older results are marked for recomputation (gate G21).
9. The independent reference in `validation/` is changed only with a derivation from the specification or first
   principles written into the diff, never from the engine's code; every planted fault must still be detected.
10. This branch only. No pull requests, tags or releases. Do not edit `marketing/`.
11. Report honestly: failures with their output; "not run" is never "passed".

## B. Before every push — run and report

`cargo build --release` · `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` ·
`cargo fmt --all -- --check` · `bash tests/e2e_smoke.sh` · `bash tests/security_smoke.sh` ·
`node tests/ui_smoke.js` (fresh server) · `node tests/desktop_smoke.js target/release/nf-server 8861` ·
`bash tests/offline_check.sh` · `node docs/design/tools/i18n_check.mjs` · `bash tests/notices_check.sh` ·
`python3 validation/run_all.py --spawn target/release/nf-server --quick`; the FULL corpus whenever the engine or
the reference changed. Totals at `ed4a5fb`: 263 / 89 / 66 / 194 / 18; quick 155 cases, full 1392 cases, 0 disagree.

After each task: what changed, exact totals, changed expectations, what is unfinished; update the register the
task belongs to; then push.

---

## PHASE 0 — Safety and housekeeping (do first)

### 0.1 You are working on the live server — make that safe or move
- **Why:** builds, test servers, fuzzing and security tests on the production host can slow or damage the live
  service. The rule is "do not touch the live server".
- **Do:** report to the owner (a) the user account and folder you work in, and that they are separate from the
  live service's account, data folder and key file; (b) every port the suites bind (8787–8889 are used by tests)
  and proof none is the live port 8792; (c) CPU and memory limits on your builds (`nice`, `ionice`, `systemd-run`
  with `MemoryMax`/`CPUQuota`); (d) that nothing ever targets the live address.
- **Never on this host:** `stress/run.py`, `tests/security_r6.sh` against anything but your own throw-away server,
  the root part of `tests/offline_check.sh`, or any command that touches the live service, Caddy or its data.
- **Recommend to the owner:** a separate development machine. Until then, heavy runs outside working hours.
- **Done when:** the four facts are written in `docs/project/CLI_WORK_PLAN.md` under this task.

#### 0.1 — the four facts (CLI, 21 Nov 2026, host `srv2072986`, the Hostinger VPS: 8 CPUs, 31 GiB memory)

**(a) Account and folders.** The CLI works as `root` in `/root/nf-work`, its own clone of this branch. The live service
is the systemd unit `nf-server.service`: user `nexusfabric`, binary `/usr/local/bin/nf-server`, data
`/var/lib/nexus-fabric`, key `/etc/nexus-fabric/master.key`, listening on `127.0.0.1:8792`, hardened by its unit
(`ProtectSystem=strict`, `ProtectHome=true`, `PrivateTmp`, `MemoryMax=1G`). It was built from the separate clone
`/root/nexus-fabric-`; backups are in `/root/backups/nexus-fabric`. **Stated plainly: `root` is not separated from
the live service by privilege; root could read or change all of it.** The separation is therefore made structural:
every build, test suite, validation run and throw-away server of this work runs through `nf-sandbox`, a host-local
wrapper (`/usr/local/bin/nf-sandbox`, not in the repository) that starts the command as its own transient systemd
service:

```
systemd-run --wait --pipe --collect --same-dir \
  -p PrivateNetwork=yes \
  -p InaccessiblePaths=/var/lib/nexus-fabric -p InaccessiblePaths=/etc/nexus-fabric \
  -p InaccessiblePaths=/root/nexus-fabric- -p InaccessiblePaths=-/root/backups/nexus-fabric \
  -p InaccessiblePaths=-/root/.config/nf-backup \
  -p ProtectSystem=full -p PrivateTmp=yes \
  -p MemoryMax=8G -p CPUQuota=400% -p Nice=10 -p IOSchedulingClass=idle -p TasksMax=4096 \
  -E CARGO_NET_OFFLINE=true -E CARGO_BUILD_JOBS=4 (+ the nf-cargo toolchain: stable, as GitHub CI) -- <command>
```

Checked from inside on 21 Nov 2026: the live data folder shows 0 entries (4 on the host) and the backup folder 0
(9 on the host); `/etc/nexus-fabric/master.key` and `/root/nexus-fabric-/Cargo.toml` are inaccessible; writing to
`/usr/local/bin` or `/etc/caddy` fails (read-only), so the live binary, the unit files and the Caddy configuration
cannot be replaced from inside. Outside the sandbox the CLI only edits files in `/root/nf-work`, runs git and reads
documents. `nf-verify <log dir>` (also host-local) runs the whole list of section B inside one sandbox and writes one
log per suite; its validation output goes to the log folder, never over `validation/results/latest.json`.

**(b) Ports.** Every port a suite or tool binds or calls by default:

| Suite or tool | Port | On this host (listening, 21 Nov 2026) |
|---|---|---|
| `tests/e2e_smoke.sh` (`NF_TEST_PORT`) | 8791 | **taken** by another service (`vnxco.web`) |
| `tests/security_smoke.sh` (`NF_TEST_PORT`) | 8792 | **the live port on this host**: the live service was installed by hand on 8792 instead of the product default 8787 |
| `tests/ui_smoke.js` (`NF_BASE`), fresh server | 8793 | free |
| `tests/desktop_smoke.js` | 8860–8869 (plan: 8861) | 8861 and 8857 **taken** (AI-OS application servers) |
| `tests/security_r6.sh` (`NF_TEST_PORT`) | 8820 | free |
| `tests/release_verify.js`, `packaging/verify/*` | 8870–8879 | free |
| `tests/offline_check.sh`, dynamic part | 8791, 8793, 8867 in its own namespace | not run on this host |
| `validation/run_all.py`, `validation/seeded_faults.py` | 8802 | free |
| `validation/exact_onsurface_check.py` | 8856, fixed (task 1.8) | free |
| `stress/run.py` / `stress/bottleneck.py` | 8810–8819 / 8816 | never run on this host |
| `docs/design/tools/seed_demo.js`, `capture_screens.js` | 8787 | **taken** by AI-OS |
| `docs/design/tools/capture_r3.js` | 8880, 8881 | free |
| `nf-server --desktop` default | 8787, then any free port | **taken** by AI-OS |
| Rust tests (`cargo test`) | `127.0.0.1:0` (ephemeral) or no socket at all | — |

Other listeners in 8700–8899 on the host: 8800 (VNX cloud). **Proof that no suite reaches the live port:** every suite
runs inside `nf-sandbox`, whose network namespace contains only its own loopback interface (`ip link` lists `lo`
only). From inside, connecting to `127.0.0.1:8792` and `127.0.0.1:8787` fails (curl exit 7, connection refused),
and binding `127.0.0.1:8792` inside succeeds because it is a different socket namespace. So the default ports above,
including 8792, are the sandbox's own: a suite can neither send a request to the live service nor occupy its port.
Outside the sandbox, `security_smoke.sh` without `NF_TEST_PORT` would send one `GET /` to the live service before its
V-38 guard refuses; it is never run that way here.

**(c) CPU and memory limits.** Read from inside the sandbox's own cgroup: `cpu.max` 400000/100000 (4 of the 8
CPUs), `memory.max` 8 GiB, `pids.max` 4096; niceness 10; I/O class idle; cargo uses 4 jobs. Investigation runs use
`NF_SANDBOX_CPU=100%` (one CPU). The live service keeps its own 1 GiB limit. A baseline of the whole list of section
B ran under these limits on 21 Nov 2026 from 00:54 UTC: build, 263 unit and integration tests, clippy, fmt, 89 API,
66 security, 194 browser, 18 desktop (on 8861 inside the sandbox), i18n, notices, offline (static part) all pass;
quick corpus 173 cases (the corpus grew with 0.4.2's boundary cases; the plan's 155 is the older count): 112 agree,
61 correctly refused, 0 disagree, 0 wrongly accepted, 0 wrongly refused, 0 errors (248 s).

**(d) Nothing targets the live address.** No suite, tool or script of this work is given `127.0.0.1:8792` or
`fabric.vsnexustechnologies.com`, and the private network namespace makes it impossible from inside the sandbox.
The CLI sends no request to the live service; its account and paths above were read from the unit file
(`systemctl cat nf-server.service`). The dynamic (root) part of `tests/offline_check.sh` is not run on this host:
`NF_OFFLINE_DYNAMIC=0` (new) runs the static part and reports the dynamic part as "not run". `stress/run.py` and
`tests/security_r6.sh` against anything but a sandboxed throw-away server are not run here.

**Recommendation to the owner.** A separate development machine (any Linux computer or a second small VPS) removes
the remaining risk, which is a mistake made outside the sandbox by an account that has root on the live host. Until
then: everything through `nf-sandbox`, and heavy runs (the full corpus, 17–25 minutes; release builds) outside the
working hours of the live server's users if the owner wants that — Japanese working hours 09:00–18:00 JST are
00:00–09:00 UTC; the baseline above ran inside them, under the limits.

### 0.2 Readiness and registers tell the truth
- `docs/project/READINESS.md`, row "Geometry verified": rewrite from the full-corpus run on 0.4.2 (1392 cases,
  973 agree, 419 refused, 0 disagree, 0 wrongly accepted, 0 wrongly refused, 27 planted faults detected). Keep it
  "not yet claimed" and list exactly what still blocks it.
- `docs/backend/DEFECT_REGISTER.md`: RB-011 still says the validation disagrees on four cases — update. Give
  every B2-00n finding a row with its status and test name. Add a section "Questions for a fabrication engineer"
  with: minimum K-factor; minimum seam clearance in the flat pattern; practical minimum and maximum branch angle;
  justification of the 0.01 mm chord limit; marking side, offset sign, rolling axis and seam conventions.
- `docs/project/NEXUS_FABRIC_COMPLETE_HANDOFF.md` section 6 heading still names commit `2894cc5`: correct it.

### 0.3 The two near-tangent FAILs (item D4)
- **Why:** you report 0 disagreements but 2 FAILs. If a job is accepted with an error bound that is not a true
  bound, a check does not mean what it says.
- **Do:** state the case ids, what fails, the measured error against the reported `eps_num`, the state the job
  ends in and whether a DXF is offered. If a file is offered while the bound is exceeded: make the job REJECTED
  (bound gate fails) until the bound is fixed in task 1.2. Add a regression case for each.

#### 0.3 — facts (CLI, 21 Nov 2026; engine `nf-engine 0.4.2+src.7d97912629c96ba8`, the build of `ed4a5fb`)

Both cases were replayed on a private server; the true errors were measured twice, independently (a first-principles
derivation at 50 digits; the repository's 3D reference at 40 digits), and an adversarial check per case re-measured
the worst points. All three agree to every printed digit. Evidence: `/root/nf-logs/0.3/` on the development host.

| | `N-tangent-r-b90-g2e-06` (r = R(1 − 2e-6), delta 0, beta 90) | `N-aspect-branch-tiny` (r/R = 1e-6, beta 60) |
|---|---|---|
| What is not PASS | `opening_developed_length_equals_3d_arc_length`, `saddle_developed_length_equals_3d_arc_length`: **INCONCLUSIVE**, not FAIL. The difference (3.9e-4 mm) is 33 × below tolerance + resolution, but the limit of the polyline length near the knee is not resolved to the required 1e-5 of the length (resolution 1.29e-2 mm, needed 1.15e-2 mm) | `opening_area_equals_surface_area`: **INCONCLUSIVE**. The verifier computes the shoelace area on absolute plate coordinates (about 1000 mm) for an opening of 3.6e-6 mm²: rounding steps of 1.16e-9 mm² against a needed 3.6e-11 mm². In coordinates relative to a vertex the same check passes (difference 3.7e-15 mm²) |
| Gate and state | G10 INCONCLUSIVE, G08 INCONCLUSIVE, G15 PASS → **REVIEW_REQUIRED** | same → **REVIEW_REQUIRED** |
| DXF offered | **yes**, both parts (HTTP 200), marked NOT RELEASED | **yes**, both parts |
| True vertex error vs reported `eps_num` | header 3.44e-12 mm vs 4.30e-11 (0.08 ×); branch 4.87e-12 vs 4.26e-11 (0.11 ×) — **holds** | header 5.23e-13 vs 7.27e-12 (0.07 ×); branch 2.73e-13 vs 1.12e-12 (0.25 ×) — **holds** |
| True chord error vs reported `eps_rep` | **exceeded**: 0.0096085 mm vs 0.0095979 mm (× 1.0011, +1.07e-5 mm) on the four knee segments of each part (phi near 0° and 180°); still below the 0.01 mm limit | header 1.09920e-8 vs 1.09920e-8 (+8.6e-14 mm, below one rounding unit of the engine's own sample, inside eps_rep + eps_num); branch within |
| Further defects found | — | the area check's tolerance is scaled to the whole plate (8.3e-3 mm², 2,284 × the opening area), so it **can never FAIL**; the reported `opening_area` quantity is 0.053 % low from the same cancellation |

The validation harness's `vertex_dev` (1.83e-7 mm and 1.35e-9 mm) is **not** a product error: it is the sagitta of the
harness's own reference polyline (refined only to 2e-10 × scale); the harness's `true_numeric_error` (hpcheck) gives the
true values above. To be corrected in task 1.8.

**Decision (CLI, under the owner's delegation of 21 Nov 2026).** The condition of this task is met for case 1 through
`eps_rep` (not `eps_num`). Instead of an interim REJECTED rule, the bound itself is fixed at once in task 1.2: report
`eps_rep` as max over segments of (largest sampled deviation + M·h²/8), where M bounds the curve's second derivative
across the chord on that segment (interval-certified) and h is the sample spacing — a derived upper bound, 0.0096401
mm here, which covers the true 0.0096085 mm and stays under 0.01 mm. Reasons: no build with the exceeded value is
deployed (the live server is not updated without the owner), the fix is small and derived, and an interim REJECT
rule could not be predicted by the independent validation oracle. The area-check defects of case 2 are fixed in the
same change (shoelace on relative coordinates, tolerance scaled to the opening, `opening_area` quantity). The two
cases become regression cases with the fixed behaviour.

---

## PHASE 1 — Mathematics and engine (first priority of the project)

### 1.1 Shallow and near-180° branch angles (owner decision pending — ask before coding)
- **Proposal:** when the verifier cannot confirm the geometry because of conditioning, answer with a refusal that
  has its own code and a plain message naming `beta_deg`, instead of an accepted job that then fails G10. Do NOT
  hard-code 0.003°: derive the condition from the verifier's own tolerance. Same logic at the obtuse end; show
  `eps_num` prominently on any accepted near-180° job.
- **Tests:** boundary cases just inside and outside at both ends; corpus cases; refusal-code table updated.

#### 1.1 — decision and result (CLI, 21 Nov 2026, engine 0.4.3)

**Decision (under the owner's delegation): the verifier's own criterion, evaluated up front, inside a derived regime.**
A pure closed-form refusal was built first and measured: the a-priori bound F on the verifier's own rounding (it grows
as u(R + r)/sin²β; derivation in `docs/backend/TOLERANCE_POLICY.md` section 7) is a worst-case sum of about twenty
roundings and runs 5 to 30 times above the error the verifier actually commits (largest measured difference / F: 0.19
saddle, 0.08 opening, 0.15 joint). As the refusal it would turn away angles the verifier confirms by a factor of 20 (for
R 150, r 100 every header with branch from about 179.97°), against the rule "never refuse a case the verifier can
confirm". So F decides only the regime: where F ≥ agree + 3·bound (a correct pattern is no longer guaranteed to pass),
the checks gate G10 reads are evaluated on the final patterns before the job is accepted, and if they fail (only the
four 3D-intersection point checks, each with its own F exceeded and a difference within F + bound; anything else is a
wrong pattern and G10 rejects it) the request is refused with `branch_angle_not_verifiable`, naming
`beta_deg`, `detail` limit = the budget, measured = F. Outside the regime a failing check is a wrong pattern and G10
rejects the job as before. No angle constant. Same rule at both ends; at the obtuse end the engine's bound grows as
1/sin²β too (rounding of β near π), so no request is refused there before the 1e8 mm size limit; instead a part whose
eps_num satisfies 8·eps_num ≥ the tolerance of its 3D point check (branch: 4·eps_num ≥ agree; header: can fire earlier, its
eps_num includes the plate estimate) carries `eps_num_governs` and the interface shows eps_num first and
in full (EN/JA). Boundaries for R 150, r 100, δ 0 (ratio F/budget 0.9): stub 0.02016°; header with branch 0.06385° and
179.96378°. Refused: stub 0.002°, header with branch 0.009°. In the regime and accepted: stub 0.003° (ratio 39), header
with branch 179.99° (ratio 1.11). Engine 0.4.3 (older results: G21 recompute). Not covered: developed-length checks that
fail for other reasons (r/R 1e-3 near 180°, near-tangent parts of 1e-3 mm) stay REJECTED (tasks 1.2, 1.6).

### 1.2 A true error bound everywhere (RB-005, D4, B2-007)
- **Why:** `eps_num` and `eps_rep` must be upper bounds, or be called estimates and not gate anything.
- **Do:** for each family write the derivation of the bound in `docs/backend/TOLERANCE_POLICY.md`; replace
  sampled estimates by a provable bound (interval evaluation of the governing expressions or an analytic bound
  on the second derivative for chord error); where no bound can be proved, the gate returns INCONCLUSIVE.
- **Tests:** property tests over random parameters comparing measured error (high-precision reference) with the
  reported bound — never above it; the two near-tangent cases; full corpus.

### 1.3 Every convention stated in the DXF and the result (RB-002, RB-003, RB-007, RB-009, RB-008 remainder)
- **What to state, for every part:** viewing/marking side; sign of the offset δ and which way positive is; the
  rolling axis; which edges are seams and which are ends; the angular datum (where 0° is) and the direction of
  increasing angle; the plate origin and axes; that the cut lines are the theoretical line with no kerf
  compensation.
- **How:** the same scheme as `docs/backend/DXF_METADATA.md`: stable `KEY: value` lines in the 999 block AND a
  TEXT entity on layer `NF_NOT_RELEASED`; the same facts in the job result and the interface (English and
  Japanese). Make the outer profile one closed polyline where the format allows (RB-008).
- **Quality gate:** reads every statement back and compares it with the job; a missing, altered or contradictory
  statement fails the gate and the file is withheld.
- **Tests:** a statement test per family and part; tampered-file tests; cut geometry byte-identical (snapshot
  test); round trip through the independent reader. Bump the engine version.

#### 1.3 — decision and result (CLI, 22 Nov 2026, engine 0.4.5)

**Built.** Every part states its plate conventions: `NF-VIEW-SIDE`, `NF-PLATE-AXES`, `NF-ROLLING-AXIS`, `NF-SEAM-EDGES`,
`NF-END-EDGES`, `NF-ANGLE-DATUM`, `NF-ORIGIN`, `NF-KERF`, `NF-STATUS` (all parts), `NF-OFFSET-DELTA`/`NF-OFFSET-SIGN` (eccentric
reducer, branch stub, header plate) and `NF-BRANCH-ANGLE` (branch stub, header plate), as 999 keys and as TEXT on NF_NOT_RELEASED.
The job result carries them in `dxf_output.statement`. The quality gate rule `convention_statement` reads them back and fails a
missing, altered, repeated, extra or contradicting statement; the file is withheld. The viewing side is the existing marking side.
Key lists per family: `uem::verify::required_convention_keys`. Format: docs/backend/DXF_METADATA.md, "Plate conventions".

**RB-008.** The outer profile of a part made of lines and polylines (cylinder shell, eccentric reducer, branch stub, header plate)
is ONE closed polyline with the same vertices in the same order. The cone frustum keeps one entity per curve: its profile has
arcs, and an arc needs a bulge encoding this build does not write, while chords would change the cut.

**Geometry.** Checked on the 156 accepted corpus DXFs before and after (dumps of the ENTITIES section up to the first TEXT): 55
cone parts byte-identical; 101 changed files, all in grouped families, each checked: one closed outer polyline, vertices equal
to the old curve chain vertex for vertex, every opening entity byte-identical. The compat fixture changed `dxf_cut_geometry`
for 93 of 335 cases (34 shell, 40 eccentric reducer, 11 branch stub, 8 header with branch); no state, outcome, refusal, parse
result or `patterns_sha256` changed. Deliberate; the fixture was regenerated and documented in compat.rs.

**Changed expectations (A2), old → new:**
1. `nf_dxf::quality::RULES`: 13 → 14 rules (test `well_formed_files_pass_every_rule`, reason: rule `convention_statement`).
2. `tests/dxf_statement.rs`, every family: the 999 keys were exactly the units and the joint keys; now the same keys plus the
   convention keys. The test compares the units and joint keys (filtered); the convention keys are checked in `tests/plate_conventions.rs`.
3. `tests/compat.rs` fixture: `dxf_cut_geometry` of 93 accepted cases changed (RB-008 grouping, listed above).
4. `ENGINE_VERSION`: `nf-engine 0.4.4+src.…` → `nf-engine 0.4.5+src.…`. Results of 0.4.4 are marked for recomputation (G21).
5. DXF `$EXTMIN`/`$EXTMAX`: the fine-print lines extend the drawing extents below the geometry in every file (the gate's
   `extents` rule still passes; the file is not the same bytes in its header).
6. Edited by the integration (the coordinator's instruction; part of the 1.3 gate, not outside it): `crates/nf-server/src/tests.rs:2498`, `0.4.4` → `0.4.5`; `tests/e2e_smoke.sh` line 81, exact TEXT count on NF_NOT_RELEASED for the cone case 3 → 12 (the three fixed lines, then the nine base labels once each; the per-label checks are kept).
7. The entity counts changed with the RB-008 grouping: `dxf_output.entities` and `reimport_report.entities_written`: cylinder shell 4 → 1; eccentric reducer 4 → 1; branch stub 4 → 1 (also the branch part of a header with branch); header plate 5 → 2 (the outer profile and one opening).
8. The marking-side TEXT line was exempt from the convention check; it is now compared with `NF-VIEW-SIDE` (one line, the declared side). A file whose marking side contradicts its viewing side fails `convention_statement`; test `a_marking_side_text_that_contradicts_the_view_side_fails_the_gate` fails on the pre-fix code and passes now.
9. `validation/checks/product.py` (the independent reader): pairs each DXF entity with an outline curve by index, so the RB-008 closed polyline broke it (KeyError in the quick run). Now `split_grouped_outline` splits a grouped outer profile back into the job's curves at the shared vertices and checks every piece against the curve's own JSON points; a mismatch raises. Derivation in its docstring. Quick corpus after the change: 177 cases, 116 agree, 0 disagree; the quick run exited PASS (its planted-fault stage was not separately inspected).

**i18n keys needed (web, EN and JA; the DXF annotations stay English, as before):** `conv.view_side`, `conv.plate_axes`,
`conv.rolling_axis`, `conv.seam_edges`, `conv.end_edges`, `conv.angle_datum`, `conv.origin`, `conv.kerf`, `conv.status`,
`conv.offset_delta`, `conv.offset_sign`, `conv.branch_angle` (the twelve labels, in docs/backend/DXF_METADATA.md) and the
family sentences of `conventions_for` (their JA is not yet written; EN source is in job.rs). Two fixed statements (KERF, STATUS,
and the branch offset sign) have their JA in the same document.

**Open.** Japanese for the family sentences (i18n above). The eccentric reducer's origin statement names the apex but gives no
distance: no such quantity is in the pattern, and none is invented. Statements are carried from the wording of develop.rs
(unreviewed; the convention status says so). CAM acceptance not tested.

### 1.4 Surface attribute for every family; "unknown" never switches a check off (RB-010)
- Cone, eccentric reducer and cylindrical shell get the same surface declaration as the branch families
  (`outside`, `inside`, `neutral`, `unknown`).
- With `unknown`, radius checks are reported INCONCLUSIVE with the reason, never silently skipped.
- Old requests without the field keep their exact behaviour and geometry (compat snapshot).

### 1.5 Checks that do not depend on the construction (RB-006)
- A mating test that works from the two DXF files alone: re-read both files with the independent reader, fold
  them back using only the stated conventions, and confirm the two cut edges coincide. This also proves the
  convention statements of 1.3 are sufficient to build the part.
- Keep the verifier's code free of calls into `develop.rs`; add a test that fails if it imports them.

**Result (done, engine 0.4.8).**

*Independence (the second bullet).* RC-15: the 3D-intersection tolerances called `develop::*_error_bound`;
`verify.rs` now re-derives the same first-order bounds itself (`engine_*_error_bound`, same Spec 02 evaluation
sequence and op order, local constants). `develop` keeps its copies for the reported eps_num; `Branch`,
`ObliqueCone` and `MAX_CHORD_ERROR_MM` moved to a neutral geometry module. The guard
`crates/uem/tests/verifier_independence.rs` checks both halves: non-test verifier code free of `develop`
imports/calls, and the two bound copies bit-equal on 2000 seeded parameter sets. TOLERANCE_POLICY section 12.

*Mating test (the first bullet).* `validation/checks/mating.py`, wired into `run_all.py` stage 0 beside the
seeded-fault self-test (its verdict is part of `selftest_ok` and `latest.json`). From the two DXF files alone:
an independent minimal DXF reader (the JSON curves only cross-check the DXF), fold-back re-derived from Spec 02
section 4.4 and the stated conventions only (full derivation in the module docstring, rule A9), the vertex
correspondence SEARCHED over both directions and all cyclic shifts, the one unstated placement (the station)
fitted and checked against the SUBMITTED request. Scope: neutral surfaces, joint not declared (one curve of
the entered radii); every load-bearing 999 statement parsed and cross-checked, missing/contradictory fails
closed. Tolerance is a sensitivity bound of the three-decimal statement rounding (5D + 1e-9 x size, derived,
not fitted; TOLERANCE_POLICY section 13). Measured on REQ (R 150 / r 100 / beta 60 / delta 20, station 500):
residual 1.36e-4 mm <= tol 2.64e-2 mm, fitted station 500.000000 mm; a rounded-input job (all four stated
channels rounded) exercises the bound; six planted faults — mirrored saddle, scaled opening, shifted saddle,
and beta/delta/circumference lies written coherently into the 999 lines — all FAIL by >= 65x the bound. This
also proves the 1.3 convention statements suffice to fold the plates back without any engine knowledge.

*Review C fixes, all in engine 0.4.8 (version history in `nf-core/src/lib.rs`, one bump):*

| Finding | Fix | Commit |
|---|---|---|
| RC-09 | cylinder pipeline runs `verify_cylinder_solid` + two plate-axis datum checks; a plate turned 90 degrees FAILs; the three solid radius checks join the unknown-surface demotion | `0ab8da0` |
| RC-10 | cone/eccentric: exact drawn signed area against the 3D surface, directed (Kabsch) apex-free congruence, seam datums, cone CCW guard. Reducer `area` is eps_rep-based and uncertified for this family -> INCONCLUSIVE by design (FAIL stays FAIL); cone `area` gains the written-vertex term. TOLERANCE_POLICY section 11 | `409e602` |
| RC-13 | self-intersection checks flag touching and short overlaps (endpoint-to-segment distance <= tol, both directions); strengthening only | `da43b07` |
| RC-15 | verifier re-derives the engine-error bounds, bit-equal (above) | `7fee9e9` |
| RC-16 | docs-only: the shared-frame convention boundary stated UNVERIFIED in THICKNESS_MODEL_DESIGN 1.4 (fold-back and Joint3 share the Spec 02 frame; a common convention error passes every interface check) | `5f287a4` |
| RC-29 | test gap closed: a mirrored eccentric reducer now FAILs CCW, signed area, directed congruence and both seam datums; the length checks keep their genuine verdicts | `447cfd5` |
| RC-32 | false `eps_rep` doc comments fixed (certified bound where provable, estimate otherwise; doc-only) | `6f18b26` |

Changed expectations are enumerated old -> new in TOLERANCE_POLICY sections 11 and 12 (compat_verdicts:
35 cylinder cases gain five checks, 55 cones and 40 reducers gain two each, reducer `area` PASS ->
INCONCLUSIVE in all 40; zero removals; 335 cases). Cut geometry is byte-identical to 0.4.7 (`develop.rs`
unchanged). Results of 0.4.7 are REJECTED with G21 recompute. The external-review loop ends with the register
updated; remaining register rows are the fabrication-engineer questions and the stated UNVERIFIED boundary.

### 1.6 Sampling density (RB-004)
- Place polyline vertices at tangent points and at parameter values where curvature is extreme, so the curve's
  key points are exact vertices. Record in the fabrication list that the 0.01 mm chord limit has no engineering
  source. No change of the limit without the owner.

### 1.7 Set-on joint, remaining case (RB-001)
- The set-on with the saddle on the branch OUTSIDE surface needs the wall thickness, which is not modelled.
  Do not approximate it. Write `docs/backend/THICKNESS_MODEL_DESIGN.md`: what inputs would be needed, what each
  part's contact surface becomes, what must come from a fabrication engineer. No code until the owner approves.

### 1.8 Validation as a permanent gate
- CI: keep the quick corpus on every push; add the FULL corpus as a scheduled (nightly) and manually triggered
  job. Decide, with a derivation, whether to tighten the scale tolerance that was relaxed to 2e-9 relative.
- Fix: `validation/run_all.py` leaves its server running if an exception escapes the planted-fault stage;
  `validation/exact_onsurface_check.py` uses a fixed port.
- Add corpus cases for every new rule of phase 1, just inside and just outside each bound.

### 1.9 Third independent review
- After 1.1–1.6: have a reviewer with no knowledge of the code derive the five developments again and test the
  build against a high-precision reference (as `docs/backend/REVIEW_B2_COMPUTATIONAL.md` did). Target: no finding
  above "low". Fix what it finds before phase 3 work on the engine.

---

## PHASE 2 — Security and reliability

### 2.1 The five failing audit checks (`tests/security_r6.sh`: 35 pass, 5 fail) — fix the product, never the test
- **V-45:** remove `'unsafe-inline'` from the style policy: move every inline `style=` and injected style into
  classes in `web/styles.css`; the canvas and SVG keep working. Browser suite must stay green in both themes.
- **V-47:** refuse a state-changing request whose `Origin` has a non-web scheme (`file://`, `data:`…) or the wrong
  scheme for the listener; keep desktop mode working (its origin is `http://127.0.0.1:port`).
- **V-44:** `deploy/install.sh` must not pipe a download into a shell and must not build as root: require an
  existing toolchain or a verified package, build as an unprivileged user, install with the least privilege.
- **Tests to add:** cross-origin requests; invalid and missing Origin; malformed installer input; insufficient
  privileges; an interrupted install; rollback leaves the previous version working.
- **Done when:** the audit reads 40 of 40 and `docs/security/VULNERABILITY_REGISTER.md` is updated.

### 2.2 Architecture review findings (`docs/backend/REVIEW_A_ARCHITECTURE.md`, RA-001 … RA-017)
- Read all seventeen; give each a status row. Fix in this order: RA-003 (the audit chain can be truncated and
  re-signed — anchor the chain so removal of the newest entries is detected), RA-002 (sign-in race), RA-001
  (connection exhaustion: a per-client connection cap, header and body read deadlines, a bound on idle
  keep-alive), RA-006 (lockout cannot be used to lock out the administrator), RA-007 (after an engine upgrade the
  interface offers a one-click recompute of affected jobs; old results stay as records).
- Each fix has a test that fails before it and passes after.

### 2.3 Stress findings (`docs/backend/STRESS_AND_RECOVERY_REPORT.md`, addendum) — on a non-live machine only
- Connection flood stalls the health check for 10 s; a restart on a still-full disk fails; two malformed
  requests are held open instead of refused. Fix, then re-run `stress/run.py` levels 4 and 6 and record the
  figures before and after.

### 2.4 Desktop security wording (owner-approved text: `docs/security/DESKTOP_KEY_ASSESSMENT.md` section 6)
- `packaging/INSTALL.md`, `packaging/README.txt` and the first-run screen in desktop mode, English and Japanese:
  the program does not encrypt the database or the whole data folder; stored files have their own encryption
  but that does not make the data set encrypted; security depends on the computer; recommend disk encryption,
  individual accounts, restricted access to the data folder and backups. Must not suggest that an
  operating-system account encrypts the database. The server needs to tell the client it is in desktop mode.

### 2.5 Database encryption and key protection — DESIGN ONLY
- Write `docs/security/DESKTOP_ENCRYPTION_DESIGN.md`: threat model (a copied data folder is in scope);
  encryption of the SQLite database; keys wrapped by the operating system (Windows DPAPI, macOS Keychain, Linux
  Secret Service, with a stated fallback); a recovery key held by the customer organisation's administrator;
  migration of existing installations that can be interrupted and resumed; backup and restore on another
  machine; key rotation; what cannot be tested without real Windows and macOS machines.
- No code until the owner approves the design.

### 2.6 Sign-in hardening — DESIGN NOTE ONLY
- V-23: multi-factor sign-in and account recovery that work fully offline (time-based codes, recovery codes).
  One page of options and costs for the owner.

---

## PHASE 3 — Product completeness (backend and frontend)

### 3.1 PDF → DXF inside the product (the owner's requested feature)
- **Read:** `docs/backend/PDF_TO_DXF.md` and `crates/nf-pdf`.
- **Server:** an endpoint that converts an uploaded PDF document by launching `nf-pdf2dxf` as a child process
  with no inherited environment, a fresh temporary working directory, no network (`unshare -n` where available),
  a kill timeout and memory limit. Inputs: document id, page, scale (millimetres per point or 1:N — required),
  tolerance, options. Store the DXF and the JSON report as documents (encrypted like other documents); record
  the conversion (source hash, page, scale, converter version) as an append-only record with an audit entry.
  Refusals keep the converter's code and message. State honestly what is not isolated on Windows, and compile
  and test the converter for Windows in the release workflow.
- **Interface:** on Projects & Documents, a "Convert to DXF" action on a PDF: page and scale form, the report
  (counts, bounding box, warnings, skipped text), a preview drawn from the DXF, download. English and Japanese.
  The result is labelled "Converted from PDF — not verified — not released" and is never shown as a flat
  pattern or attached to a calculation.
- **Tests:** API (success, every refusal code, every limit, wrong role); browser in both languages; a test that
  the child really has no network and is killed on timeout; the fuzz corpus run through the server path.
- **Close the converter's own gaps first:** the operand limit has no test; `--no-text` has no test; the 64 MiB
  input limit is only tested with a lowered value.

### 3.2 Forms and refusals
- BH-014: when the user returns to the form ("New from these values") every refused field is marked with the
  message for its refusal code, in both languages, for all 41+ codes.
- The refusal-reason minimum length counts characters, not bytes (a two-character Japanese reason passes
  today). Same for any other length rule.

### 3.3 Words and language
- A test that fails if the interface, packages or install texts contain any word of rule A.3.
- Confirm every gate G00–G22, every check name, every refusal code and every record kind has a Japanese label;
  the Japanese walk in `tests/ui_smoke.js` fails on any untranslated identifier outside the documented list.
- Keep the disclosure "EV-1 (AI-generated, not independently reviewed)".

### 3.4 Speed and scale — measure before and after, no behaviour change
- Listing endpoints grow with the total number of jobs: add pagination with indexed queries.
- Audit verification took 1.6 s at 20,000 jobs: verify incrementally from the last verified anchor, full
  verification on demand.
- One database mutex limits throughput with many users: separate read connections (WAL allows it), writes
  stay serialised.

### 3.5 Interface quality
- Find the cause of the intermittent browser-check failure seen twice (a page-wait race in the viewer-role
  check) and fix the wait, not the assertion.
- Keyboard and screen-reader pass on the new-calculation form, the job tabs and the conversion screen.
- Offline help in English and Japanese inside the program: what each part family needs, what each gate and
  refusal means, what is not modelled. Three sample projects.

---

## PHASE 4 — Release

### 4.1 Reproducible builds
- Two builds of one commit give different checksums. Remove the causes (timestamps in the zip and the
  installer, path prefixes, non-deterministic ordering) so a checksum identifies a commit; if one cause cannot
  be removed, the manifest says which.

### 4.2 Release verification
- `packaging/verify_release.sh` on the release commit: build from a clean copy, checksums, version agreement
  (installer name, `Cargo.toml`, `--version`, engine identifier), Linux checks natively. A manifest row that was
  cut short by a full disk in the rehearsal must be complete this time.
- Windows: `docs/release/HANDOFF_WINDOWS_VERIFICATION.md` on a real Windows PC (not on the VPS).

### 4.3 Continuous integration
- Both jobs green on GitHub (`software`, `geometry-validation`); the release workflow run once by manual
  trigger to prove it builds on Windows and Linux runners (artefacts only; no release, no tag).

### 4.4 Release-candidate report
- `docs/release/RC1_REPORT.md`: source commit; artefacts with sizes and SHA-256; every suite's result; the five
  readiness classifications with evidence; known limitations; open security findings; install instructions;
  what was run natively and what only under an emulator.

### 4.5 Legal drafts (for the owner's lawyer; each marked "DRAFT — not reviewed by a lawyer")
- Terms of use, privacy statement (no telemetry; data stays on the customer's machine), engineering
  responsibility notice, in English and Japanese. See `docs/legal/LEGAL_READINESS.md`.

### 4.6 The live server
- Updated only when the owner says so, by pulling the release commit, rebuilding and restarting, with
  `NF_ALLOWED_HOSTS=fabric.vsnexustechnologies.com`. Back up the data folder and key first; keep the previous
  binary for rollback; check `/api/health` and the audit verification afterwards.
- DRAFT (22 Nov 2026, not run): the step-by-step live upgrade to the RA-003 / RA-006 build is in
  `deploy/README.md`, section "Live upgrade to the RA-003 / RA-006 build". It needs `NF_ACCEPT_UNANCHORED_AUDIT=1` for one
  start (the live log has no anchor), then the flag is removed; `NF_TRUSTED_PROXY=127.0.0.1` is required in the unit.

---

## PHASE 5 — Later; ask the owner before starting any of these

1. Plate thickness, neutral axis, bend allowance, weld gap, kerf — needs process values from a fabrication
   engineer; design first (`THICKNESS_MODEL_DESIGN.md`).
2. More part families: segmented elbows, rectangular-to-round transitions, Y-branches, several branches on one
   header. Each: derivation written first, independent reference before the engine, regression cases, gates.
3. 3D view of the folded part and the joint.
4. PDF pattern sheet (dimensions, title block, status NOT RELEASED), cutting list, project export.
5. A desktop window instead of the browser tab.
6. Engineer approval workflow (roles, signature, obligations closed, replay, audit) — the only way a result may
   ever leave NOT RELEASED; design reviewed by the owner before any code.
7. DXF import (isolated parser, fuzz tests), then nesting on sheets and CNC output.

## PHASE 6 — What only people can supply (track, do not attempt)

- A fabrication engineer: the questions in task 0.2, and a real cut part compared with a pattern.
- A lawyer admitted in Japan: terms, liability limits, privacy, status of AI-assisted code; a trademark search
  for "NEXUS FABRIC" and "AMOEBA".
- A person at a real Windows PC and a real Mac.
- A native-speaking engineer's review of every Japanese text.

---

## Order of work

0.1 → 0.2 → 0.3 → 1.1 (after the owner decides) → 1.2 → 1.3 → 1.4 → 1.5 → 1.6 → 1.8 → 1.9 → 2.1 → 2.2 → 2.4 →
3.1 → 3.2 → 3.3 → 3.4 → 3.5 → 2.3 (non-live machine) → 4.1 → 4.2 → 4.3 → 4.4 → 4.5.
Design-only documents (1.7, 2.5, 2.6) can be written at any point while a long test run is going.
Stop and ask the owner on: any engine-version bump, any change to what the engine accepts, any design document
awaiting approval, anything that needs a fabrication engineer's value, and before touching the live service.
