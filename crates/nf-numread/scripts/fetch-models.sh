#!/bin/sh
# Fetch the nf-numread model files from their official source and verify SHA-256.
# usage: fetch-models.sh [target-dir]   (default: crates/nf-numread/models next to this script)
# Needs curl and sha256sum. Download only; nothing is executed.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
dir=${1:-$here/../models}
mkdir -p "$dir"
# RapidOCR model repository on Hugging Face (Apache-2.0), pinned to a commit; the ONNX files are
# PaddleOCR PP-OCRv4 (ch, mobile) exported to ONNX by the RapidOCR project.
HF=https://huggingface.co/SWHL/RapidOCR/resolve/1cfba2e90fc938db55889873735088de210cc173/PP-OCRv4
# Dictionary from the PaddleOCR repository, release/2.7.
KEYS=https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/ppocr_keys_v1.txt
fetch() { # name url sha256
  out=$dir/$1
  if [ -f "$out" ] && [ "$(sha256sum "$out" | cut -d' ' -f1)" = "$3" ]; then echo "ok (present) $1"; return; fi
  tmp=$out.part
  curl -fsSL --retry 3 -o "$tmp" "$2"
  got=$(sha256sum "$tmp" | cut -d' ' -f1)
  if [ "$got" != "$3" ]; then rm -f "$tmp"; echo "SHA-256 MISMATCH for $1: got $got want $3" >&2; exit 1; fi
  mv "$tmp" "$out"; echo "ok (fetched) $1"
}
fetch ch_PP-OCRv4_det_infer.onnx "$HF/ch_PP-OCRv4_det_infer.onnx" d2a7720d45a54257208b1e13e36a8479894cb74155a5efe29462512d42f49da9
fetch ch_PP-OCRv4_rec_infer.onnx "$HF/ch_PP-OCRv4_rec_infer.onnx" 48fc40f24f6d2a207a2b1091d3437eb3cc3eb6b676dc3ef9c37384005483683b
fetch ppocr_keys_v1.txt "$KEYS" 28b2362ad4ab2dc38769aa72feb535e3a9ddb3fd2a7585a05920e6393b1dc7f7
