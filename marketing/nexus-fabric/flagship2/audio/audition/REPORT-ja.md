# Voice audition, ja: every candidate read the whole script at its default speed

Recogniser: faster-whisper `large-v3-turbo`. Error = word error (en) or kana error (ja) against the script; confidence = the recogniser's mean log-probability (closer to 0 is clearer). PESQ/STOI are torchaudio SQUIM estimates of signal cleanliness, not naturalness. Nobody listened.

| Voice | Error | Lines exact | Confidence | Pace, kana/s | Median pitch, Hz | Pitch range, st | Pitch s.d., st | Pauses | PESQ est. | Clipped |
|---|---|---|---|---|---|---|---|---|---|---|
| jf_gongitsune | 0.017 | 6/9 | -0.14 | 7.81 | 235.3 | 9.86 | 3.13 | 31 × 0.46 s | 3.92 | 0 |
| jf_alpha | 0.031 | 4/9 | -0.09 | 8.01 | 271.2 | 9.02 | 2.97 | 16 × 0.34 s | 3.73 | 0 |
| jm_kumo | 0.031 | 4/9 | -0.12 | 7.95 | 98.2 | 9.14 | 3.13 | 28 × 0.30 s | 3.99 | 113 |
