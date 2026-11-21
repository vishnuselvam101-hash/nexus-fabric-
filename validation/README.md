# Independent computational validation

Python suite that checks NEXUS FABRIC results against a separately written reference. Report: `docs/backend/COMPUTATIONAL_VALIDATION_REPORT.md`.

```
pip install -r validation/requirements.txt
python3 validation/run_all.py --spawn /path/to/nf-server          # private server, temp data dir, port 8802
python3 validation/run_all.py --url http://host:port --user U --password P [--setup-token T | NF_SETUP_TOKEN=T]   # token only for a server with no administrator yet
python3 validation/run_all.py --spawn target/release/nf-server --quick   # CI: ~2 min, keeps Defect-002 / marking-side / DXF-tamper detectors
python3 validation/seeded_faults.py --url ...                      # tests the validator itself (includes Defect 002)
python3 validation/workload_gen.py --parts 2000 --seed 7 --invalid 0.05 --out workload.jsonl
python3 validation/analysis_view_side.py                           # size of the error if a pattern is marked on the wrong face
python3 validation/corpus/build_corpus.py                          # regenerates corpus/cases (about 10 minutes, mpmath)
```

`run_all.py` exits non-zero on any mismatch or defect finding and writes `results/latest.json`.

| Path | Content |
|---|---|
| `reference/ruled.py` | generic development of ruled developable surfaces (geodesic-curvature integration), triangulation unroller, rigid fitting |
| `reference/families.py` | 3D construction and reference pattern of each family; header/branch placed in an oblique world frame; intersection by bisection |
| `reference/hp.py` | mpmath tier (40 digits) |
| `checks/product.py` | job JSON + DXF model, two DXF readers (ezdxf, own parser), measures from the DXF |
| `checks/compare.py` | DXF round trip, frame mapping with and without mirroring, fold-back, 3D mating |
| `checks/hpcheck.py` | true numerical error against mpmath, versus stated `eps_num` |
| `checks/oracle.py` | independent accept / refuse decision |
| `corpus/cases/*.json` | 518 cases with expected values and derivations; `corpus/INDEX.json` |
| `stability.py` | near-tangent, shallow, extreme aspect ratio and magnitude cases |
