# NFJ-1 R0 spike: offline number reader (2026-11-27)

Question: can an offline, CPU-only, no-training reader find every number on a drawing page with its box and text as printed?
Answer: yes on our synthetic pages (PP-OCRv4 det+rec on the pure Rust `rten` runtime, 95.1 % exact on the 20-page sample,
0.4 false reads per page). Not shown on real partner drawings (no truth exists for them).

## Data and metric
- sample: 20 synthetic pages with generator truth (/root/nf-data/numread-spike/sample, 345 numeric items).
- smoke: 40 further synthetic pages with truth (/root/nf-data/reader-synth/v1-fix-smoke/test, 705 items), same generator.
- public eval set (/root/nf-data/reader-eval): 5 pages run for timing and a look only. **It has no truth, so nothing is measured on it.**
- Scorer: tools/score.py. exact = same text (φ/Φ/Ø/⌀ folded) with box IoU >= 0.5; value-only = same digits, IoU >= 0.3;
  missed; false = tokens that match no truth item (inside a truth item's box = "wrong", elsewhere = "spurious").

## Engines (sample, 20 pages, 345 items)
| engine | exact | value-only | missed | false / page | CPU-s / page | licence |
|---|---|---|---|---|---|---|
| **PP-OCRv4 det+rec, rten, nf-numread (final, phi 0.05)** | **95.1 %** | 0.6 % | 4.3 % | **0.4** | 8.0 (10.1 s wall, load 15-21) | models Apache-2.0, rten MIT/Apache-2.0 |
| same, phi repair off (phi 0) | 93.6 % | 2.0 % | 4.3 % | 0.4 | 8.0 | same |
| spike binary, same det + rec, plain decode, 1920 px | 67.0 % | 18.0 % | 15.1 % | 0.3 | 3.6 | Apache-2.0 |
| spike binary, same, 2560 px | 67.8 % | 14.5 % | 17.7 % | 0.7 | 5.0 | Apache-2.0 |
| PP-OCRv4 det + en_PP-OCRv3 rec | 49.3 % | 23.2 % | 27.5 % | 3.6 | 3.2 | Apache-2.0 |
| PP-OCRv4 det + japan_rec_crnn | 6.1 % | 2.6 % | 91.3 % | 5.0 | 1.9 | provenance unknown, rejected |
| ocrs (Rust, rten) | 32.5 % | 2.6 % | 64.9 % | 8.1 | 1.9 | engine MIT/Apache-2.0, **models CC-BY-SA-4.0** |
| Tesseract 5.5 (psm 11, eng+jpn) | 50.4 % | 1.2 % | 48.4 % | 5.2 | not measured (100 s wall mean, p50 32 s, under load) | Apache-2.0 |
Held-out smoke (40 pages, 705 items, never used to tune): nf-numread final 90.9 % exact, 3.0 % value-only, 6.1 % missed, 0.6 false/page,
9.6 s/page wall; ocrs 32.6 % exact, 8.7 false/page.

Final nf-numread by class (sample): plain 96 %, φ 97 % (28 of 29), ± 91 %, R 86 % (n=7), ° 80 % (n=5; the rest value-only).
By type: title 95 %, dimension 96 %, BOM cells 93 %.

## phi sweep (diameter sign repair, `Params::phi_thresh`, sample)
| phi_thresh | exact | value-only | missed | φ items exact |
|---|---|---|---|---|
| 0 (off) | 93.6 % | 2.0 % | 4.3 % | 79 % |
| 0.02 | 95.1 % | 0.6 % | 4.3 % | 97 % |
| **0.05 (default)** | 95.1 % | 0.6 % | 4.3 % | 97 % |
| 0.1 | 94.8 % | 0.9 % | 4.3 % | 93 % |
| 0.2 | 94.8 % | 0.9 % | 4.3 % | 93 % |
False reads are 0.4 per page at every setting. The sweep was run on the sample pages, the same 20 pages on which the repair rules
were developed, so 95.1 % is a tuned figure; the smoke figure (90.9 %) is the better estimate for unseen synthetic pages.

## Honest notes
- Timing was measured while the host was loaded (load average 15-21, 4 cores via taskset, nice 10). Wall seconds are inflated;
  CPU-seconds (user time / pages) are the steadier number. Peak RSS about 370-400 MB. An idle-machine timing was not taken.
- The pages are synthetic (our generator, JA/EN factory style, scanned look). Real partner drawings may be harder (stamps, hand
  notes, curved text, heavy scan noise). The public eval set has no truth: on 5 pages the reader returned 22, 7, 1, 1, 1 items
  (9-39 s per page under load); that shows it runs on real pages, not how accurate it is.
- Misses are mostly small or vertical text and tokens touching dimension lines; the remaining value-only items are the degree sign.
- Reading is advisory: the reader proposes numbers, a person or the verifier (R1) confirms. It never approves a value.
- Tesseract ran with OMP_THREAD_LIMIT=4, sequentially; wall time only (CPU-s not recorded). It reads no φ or ± (0 % and 0 % exact).
- Model provenance: the ONNX bytes are identical to the files in the RapidOCR Hugging Face repository (Apache-2.0), commit
  1cfba2e9; the link back to the official Paddle release is by RapidOCR's export, not a hash (models/MODELS.md).

## Recommendation
Ship PP-OCRv4 det+rec (Apache-2.0) on `rten` as `nf-numread`, models fetched and SHA-256 pinned, not in git. Keep ocrs out
(32.5 % exact, ShareAlike model licence). Next: R1 uses the item list (text, kind hint, box) with the BOM and the engine as the
verifier; measure on a few real drawings with hand-made truth before promising accuracy on them; try the larger server models
(RapidOCR also hosts `ch_PP-OCRv4_*_server_infer.onnx`) if misses on real pages matter more than time.

## Review round 1 (2026-11-27, build3)
Rules changed so that no wrong number is reported silently: ':'/';' becomes a decimal point only with 3+ digits before and 1-2 after
(1:2 and 12:30 give no value, flagged ambiguous); a leading +/- is kept (kind tolerance) only before a decimal below 10, otherwise it
is removed as a stroke and the item is flagged ambiguous (a first version that kept every sign cost 2.9 points on sample, 2.7 on smoke,
because dimension-line strokes were read as signs; it was not kept); commas follow the nf-server orders rule (thousands group -> value,
anything else -> decimal reading flagged ambiguous); an inferred phi is flagged phi_inferred; items carry repaired, ambiguous and
low_confidence (confidence < 0.85) flags. Re-scored with tools/score.py on the same sets: sample 95.1 % exact / 0.4 false per page,
smoke 90.9 % exact / 0.6 false per page: unchanged (res5, logs build3-scores.log). Timing was again taken under load (load 33-38).
