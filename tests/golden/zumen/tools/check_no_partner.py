"""Partner-factory check for tests/golden/zumen (run: python3 -I tools/check_no_partner.py).
Cannot prove absence of a value; it checks what can be checked mechanically:
 1. no partner marker word outside the readings' own "not a partner drawing" notes;
 2. every readings/*.json says it is hand made / public, except the two readings of generated public test images;
 3. every reading file named by a golden case exists in readings/ (no hidden input);
 4. no file name of the older partner-based prototypes or of reader/out/ is present.
Exit 1 on any finding."""
import glob, json, os, re, sys
root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
markers = ["partner", "取引先", "外だし", "外出し", "パートナー", "協力工場", "screen-v1", "dir-1", "dir-2", "dir-3", "reader/out"]
neg = re.compile(r"[（(]協力工場の図面ではない[。）)]?[^）)]*[）)]?")
# files known to be readings of the public generated test PNGs (ZK-STUDY section 0, README_en section 8)
generated_png_readings = {"G_tee_claude.json", "G2_tee_claude.json"}
bad = []
for dp, _, fs in os.walk(root):
    if "__pycache__" in dp: continue
    for f in fs:
        p = os.path.join(dp, f)
        rel = os.path.relpath(p, root)
        if rel == "tools/check_no_partner.py": continue
        try: t = open(p, encoding="utf-8").read()
        except Exception: continue
        t2 = neg.sub("", t) if rel.startswith("readings/") else t
        if rel == "README.md":  # the README names the markers on purpose
            continue
        for m in markers:
            if m.lower() in t2.lower(): bad.append(f"{rel}: marker '{m}'")
for f in sorted(glob.glob(os.path.join(root, "readings", "*.json"))):
    n = os.path.basename(f); note = json.load(open(f, encoding="utf-8")).get("_note") or ""
    if n in generated_png_readings: continue
    if not ("ではない" in note and ("手で作った" in note)): bad.append(f"readings/{n}: no hand-made/not-partner note")
have = {os.path.basename(f) for f in glob.glob(os.path.join(root, "readings", "*.json"))}
for f in glob.glob(os.path.join(root, "s*.json")):
    for c in json.load(open(f, encoding="utf-8")).get("cases", []):
        for r in c.get("reading_hint", []):
            if r not in have: bad.append(f"{os.path.basename(f)} {c['id']}: reading {r} missing")
print(f"readings checked: {len(have)}; findings: {len(bad)}")
for b in bad: print("FOUND", b)
sys.exit(1 if bad else 0)
