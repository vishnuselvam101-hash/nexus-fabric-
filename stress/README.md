# NEXUS FABRIC stress framework
Python 3 + psutil. Run with your own copy of the binary (never the shared build tree):

    cp target/release/nf-server /some/scratch/nf-server
    NF_BIN=/some/scratch/nf-server python3 stress/run.py --level N --duration S --port 8810 --workdir /some/scratch/w
    python3 stress/bottleneck.py --port 8816 --bin /some/scratch/nf-server --workdir /some/scratch/wbn

Levels 1-6 (see docs/backend/STRESS_AND_RECOVERY_REPORT.md). Ports are restricted to 8810-8819, the runner refuses
to use a port that is already listening, and starts/stops its own server on a disposable data dir.
Results: stress/results/*.json and *.md. `--quick` gives a small smoke run. `--level 6 --only db` runs only the DB-damage cases.
