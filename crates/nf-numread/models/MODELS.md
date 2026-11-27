# Models (nf-numread)

The model weights are **not in git**. Fetch and verify them with `scripts/fetch-models.sh [dir]`
(default dir: this directory; the `.onnx` files are git-ignored). The reader loads from
`$NF_NUMREAD_MODELS`, else this directory, and refuses any file whose SHA-256 differs from the value
below (`Reader::from_dir`; tested in `tests/reader_smoke.rs`). Packaging for the offline product
copies the three files next to the program and sets `NF_NUMREAD_MODELS`.

| file | role | size (bytes) | SHA-256 | licence |
|---|---|---|---|---|
| ch_PP-OCRv4_det_infer.onnx | text detection (DB) | 4 745 517 | d2a7720d45a54257208b1e13e36a8479894cb74155a5efe29462512d42f49da9 | Apache-2.0 |
| ch_PP-OCRv4_rec_infer.onnx | text recognition (CTC) | 10 857 958 | 48fc40f24f6d2a207a2b1091d3437eb3cc3eb6b676dc3ef9c37384005483683b | Apache-2.0 |
| ppocr_keys_v1.txt | recogniser dictionary | 26 249 | 28b2362ad4ab2dc38769aa72feb535e3a9ddb3fd2a7585a05920e6393b1dc7f7 | Apache-2.0 |

The dictionary is small text under Apache-2.0 and is kept in the repository (byte-identical to the
official file; it is also covered by the hash check). The two ONNX files are fetched only.

## Provenance (checked 2026-11-27)

- ONNX files: PaddleOCR PP-OCRv4 (ch, mobile) exported to ONNX by the RapidOCR project, published at
  https://huggingface.co/SWHL/RapidOCR (path `PP-OCRv4/`, repository licence field: apache-2.0).
  The files used in the spike were downloaded again from that repository (commit
  1cfba2e90fc938db55889873735088de210cc173) and are **byte-identical** (same SHA-256 as above, also
  equal to the LFS object ids the repository lists). The fetch script pins that commit.
- Dictionary: https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/ppocr_keys_v1.txt
  (identical entries to the spike copy, which only had one extra trailing newline).
- Not established: the repository only records that RapidOCR produced the export with Paddle2ONNX; the
  exact Paddle2ONNX version and the original Paddle inference model hash are not published there, so
  the link from these ONNX bytes back to the official Paddle `.tar` release is by RapidOCR's word, not
  by a hash we could compare. The upstream licence is Apache-2.0 (PaddleOCR, RapidOCR); keep the
  NOTICE/licence texts when the models are shipped.

## Rejected for shipping (measured in SPIKE.md)

- ocrs `text-detection.rten` / `text-recognition.rten`: CC-BY-SA-4.0 (ShareAlike on the model); 32.5 % exact.
- en_PP-OCRv3 recogniser and japan_rec_crnn: measured only; japan_rec_crnn has no recorded provenance.
- Tesseract 5.5.0 + tessdata 4.1.0 (Apache-2.0): comparison only, not shipped.
