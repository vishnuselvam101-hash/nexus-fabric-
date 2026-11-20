# Voice audition, en: every candidate read the whole script at its default speed

Recogniser: faster-whisper `large-v3-turbo`. Error = word error (en) or kana error (ja) against the script; confidence = the recogniser's mean log-probability (closer to 0 is clearer). PESQ/STOI are torchaudio SQUIM estimates of signal cleanliness, not naturalness. Nobody listened.

| Voice | Error | Lines exact | Confidence | Pace, words/s | Median pitch, Hz | Pitch range, st | Pitch s.d., st | Pauses | PESQ est. | Clipped |
|---|---|---|---|---|---|---|---|---|---|---|
| af_heart | 0.000 | 9/9 | -0.12 | 3.40 | 198.8 | 9.45 | 3.0 | 13 × 0.31 s | 3.76 | 0 |
| bf_isabella | 0.000 | 9/9 | -0.15 | 3.38 | 205.1 | 7.76 | 2.51 | 8 × 0.37 s | 3.85 | 1 |
| bm_fable | 0.000 | 9/9 | -0.16 | 2.75 | 120.8 | 16.04 | 5.19 | 0 × 0.00 s | 2.94 | 4 |
| bm_lewis | 0.000 | 9/9 | -0.16 | 3.18 | 113.5 | 11.44 | 4.14 | 22 × 0.29 s | 3.76 | 0 |
| bf_emma | 0.000 | 9/9 | -0.16 | 3.45 | 181.8 | 6.52 | 2.31 | 7 × 0.39 s | 3.85 | 1 |
| bm_george | 0.000 | 9/9 | -0.17 | 3.03 | 142.9 | 11.65 | 3.37 | 24 × 0.24 s | 3.32 | 0 |
| af_bella | 0.000 | 9/9 | -0.17 | 3.35 | 202.5 | 7.87 | 2.79 | 14 × 0.35 s | 3.57 | 0 |
| af_aoede | 0.000 | 9/9 | -0.17 | 3.31 | 177.8 | 10.0 | 3.47 | 6 × 0.28 s | 3.59 | 37 |
| am_michael | 0.000 | 9/9 | -0.20 | 3.09 | 118.5 | 10.52 | 4.33 | 12 × 0.34 s | 3.87 | 0 |
| af_nicole | 0.006 | 8/9 | -0.15 | 2.83 | 160.0 | 15.63 | 4.39 | 40 × 0.47 s | 3.01 | 0 |
| af_kore | 0.007 | 8/9 | -0.12 | 3.15 | 152.4 | 13.78 | 4.37 | 7 × 0.15 s | 3.81 | 0 |
| af_sarah | 0.007 | 8/9 | -0.15 | 3.51 | 193.9 | 12.32 | 3.67 | 13 × 0.34 s | 4.04 | 5 |
| am_fenrir | 0.007 | 8/9 | -0.16 | 3.22 | 135.6 | 14.47 | 4.73 | 9 × 0.18 s | 4.02 | 24 |
| am_puck | 0.007 | 8/9 | -0.18 | 3.22 | 115.1 | 13.86 | 4.83 | 3 × 0.17 s | 3.35 | 9 |
