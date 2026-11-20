#!/usr/bin/env python3
"""script/narration.json -> script/narration.js (window.NF_SCRIPT) for the film page (file:// pages cannot fetch JSON)."""
import json, os
H = os.path.dirname(os.path.abspath(__file__)); N = json.load(open(os.path.join(H, "..", "script", "narration.json"), encoding="utf-8"))
out = {l: {"lines": [x["text"] for x in N[l]["lines"]], "onscreen": N[l]["onscreen"]} for l in ("en", "ja")}
open(os.path.join(H, "..", "script", "narration.js"), "w", encoding="utf-8").write("window.NF_SCRIPT = " + json.dumps(out, ensure_ascii=False) + ";\n")
print("narration.js written")
