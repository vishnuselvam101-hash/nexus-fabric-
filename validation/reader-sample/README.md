20 sample pages of the synthetic drawing generator (crates/nf-reader-data), seed 1, `nf-reader-data sample`.
truth.jsonl: one JSON object per page (schema: crates/nf-reader-data/schema.json). pdf/: vector pages (unskewed).
png/: clean pages only; the 10 scan pages (noise, skew, blur, JPEG) are not stored here, regenerate them with
`nf-reader-data sample --out DIR --seed 1` (byte-identical truth).
