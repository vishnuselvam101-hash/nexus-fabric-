#!/usr/bin/env python3
"""Shared local speech tools for the flagship2 film: engines, measurement, recognition.

Everything runs on this machine. No text or audio is sent to any speech service.
Engines:  Kokoro-82M v1.0 (kokoro-onnx, Apache-2.0)            English, Japanese
          VOICEVOX CORE 0.16.3 (MIT) + VOICEVOX voice models    Japanese, with Open JTalk pitch accent
Checker:  faster-whisper (MIT code, OpenAI Whisper weights MIT) speech-recognition round trip
"""
import io, os, re
import numpy as np
import soundfile as sf
from scipy.signal import resample_poly

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 48000
KOKORO_DIR = os.environ.get("KOKORO_DIR", os.path.expanduser("~/.cache/nf-audio/kokoro"))
VV_DIR = os.environ.get("VOICEVOX_DIR", os.path.expanduser("~/.cache/nf-audio/voicevox"))

# ---------------------------------------------------------------- engines
_kokoro = None
def kokoro():
    global _kokoro
    if _kokoro is None:
        from kokoro_onnx import Kokoro
        _kokoro = Kokoro(os.path.join(KOKORO_DIR, "kokoro-v1.0.onnx"), os.path.join(KOKORO_DIR, "voices-v1.0.bin"))
    return _kokoro

_jag2p = None
def ja_phonemes(text):
    global _jag2p
    if _jag2p is None:
        from misaki import ja
        _jag2p = ja.JAG2P()
    out = _jag2p(text)
    return out[0] if isinstance(out, tuple) else out

def kokoro_say(text, voice, speed=1.0, lang="en-us"):
    """-> float64 mono at 48 kHz (model output is 24 kHz)."""
    k = kokoro()
    if lang == "ja":
        wav, sr = k.create(ja_phonemes(text), voice=voice, speed=speed, is_phonemes=True)
    else:
        wav, sr = k.create(text, voice=voice, speed=speed, lang=lang)
    return resample_poly(np.asarray(wav, dtype=np.float64).flatten(), SR, sr)

_vv = None
def voicevox():
    """Synthesizer with every .vvm in VV_DIR/vvms loaded. Returns (synth, {style_id: 'speaker / style'})."""
    global _vv
    if _vv is None:
        from voicevox_core.blocking import Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile
        ort_dir = [d for d in sorted(os.listdir(VV_DIR)) if d.startswith("voicevox_onnxruntime")][-1]
        ort = Onnxruntime.load_once(filename=os.path.join(VV_DIR, ort_dir, "lib", Onnxruntime.LIB_VERSIONED_FILENAME))
        syn = Synthesizer(ort, OpenJtalk(os.path.join(VV_DIR, "open_jtalk_dic_utf_8-1.11")))
        names = {}
        only = os.environ.get("VOICEVOX_VVMS")
        for f in sorted(os.listdir(os.path.join(VV_DIR, "vvms"))):
            if not f.endswith(".vvm") or (only and f[:-4] not in only.split(",")):
                continue
            with VoiceModelFile.open(os.path.join(VV_DIR, "vvms", f)) as m:
                syn.load_voice_model(m)
                for sp in m.metas:
                    for st in sp.styles:
                        if int(st.id) < 3000 or int(st.id) >= 10000:      # talk styles only
                            names[int(st.id)] = f"{sp.name} / {st.name}"
        _vv = (syn, names)
    return _vv

def wav_bytes_to_48k(b):
    x, sr = sf.read(io.BytesIO(b), dtype="float64")
    if x.ndim > 1:
        x = x.mean(axis=1)
    return x if sr == SR else resample_poly(x, SR, sr)

def voicevox_query(text=None, kana=None, style_id=0, speed=1.0, pitch=0.0, intonation=1.0, pause_scale=1.0, pause_cap=None):
    syn, _ = voicevox()
    aq = syn.create_audio_query_from_kana(kana, style_id) if kana else syn.create_audio_query(text, style_id)
    aq.speed_scale = speed; aq.pitch_scale = pitch; aq.intonation_scale = intonation
    aq.pre_phoneme_length = 0.05; aq.post_phoneme_length = 0.05
    aq.output_stereo = False                                 # 24 kHz model output; resampled to 48 kHz afterwards
    for ap in aq.accent_phrases:
        pm = ap.pause_mora
        if pm is not None:                                  # the binding returns the pause as a dict or as a Mora
            get = (lambda: pm["vowel_length"]) if isinstance(pm, dict) else (lambda: pm.vowel_length)
            v = get() * pause_scale; v = min(v, pause_cap) if pause_cap else v
            if isinstance(pm, dict):
                pm["vowel_length"] = v
            else:
                pm.vowel_length = v
    return aq

def voicevox_say(text=None, kana=None, style_id=0, **kw):
    syn, _ = voicevox()
    aq = voicevox_query(text, kana, style_id, **kw)
    return wav_bytes_to_48k(syn.synthesis(aq, style_id)), aq

def aq_summary(aq):
    """Accent phrases as text: mora string with ' after the accent nucleus, plus mora count."""
    out, n = [], 0
    for ap in aq.accent_phrases:
        s = ""
        for i, m in enumerate(ap.moras, 1):
            s += m.text + ("'" if i == ap.accent else "")
        n += len(ap.moras)
        out.append(s + ("、" if ap.pause_mora is not None else ""))
    return "/".join(out), n

# ---------------------------------------------------------------- measurement
def trim(x, sr=SR, thr_db=-45.0, pad=0.03):
    env = np.abs(x); thr = 10 ** (thr_db / 20) * max(env.max(), 1e-9)
    idx = np.where(env > thr)[0]
    if len(idx) == 0:
        return x
    a = max(0, idx[0] - int(pad * sr)); b = min(len(x), idx[-1] + int(pad * sr))
    y = x[a:b].copy(); f = int(0.008 * sr)
    y[:f] *= np.linspace(0, 1, f); y[-f:] *= np.linspace(1, 0, f)
    return y

def pauses(x, sr=SR, min_len=0.10, thr_db=-38.0):
    """Internal silences [(start_s, end_s)] of at least min_len."""
    hop = int(0.005 * sr); win = int(0.02 * sr); n = max(1, (len(x) - win) // hop)
    rms = np.array([np.sqrt(np.mean(x[i * hop:i * hop + win] ** 2)) for i in range(n)])
    quiet = rms < 10 ** (thr_db / 20) * rms.max()
    gaps = []; i = 0
    while i < n:
        if quiet[i]:
            j = i
            while j < n and quiet[j]:
                j += 1
            a, b = i * hop / sr, (j * hop + win) / sr
            if b - a >= min_len and i > 0 and j < n:
                gaps.append((round(a, 3), round(b, 3)))
            i = j
        else:
            i += 1
    return gaps

def f0_stats(x, sr=SR):
    """Autocorrelation pitch track on voiced 40 ms frames. -> median Hz, 5-95 % range and s.d. in semitones, voiced share."""
    y = resample_poly(x, 16000, sr); fs = 16000
    win = int(0.04 * fs); hop = int(0.01 * fs); lo, hi = int(fs / 420), int(fs / 60)
    peak = np.abs(y).max() + 1e-9; f0 = []; frames = 0
    w = np.hanning(win)
    for i in range(0, len(y) - win, hop):
        s = y[i:i + win]; frames += 1
        if np.sqrt(np.mean(s ** 2)) < 0.03 * peak:
            continue
        s = (s - s.mean()) * w
        ac = np.correlate(s, s, "full")[win - 1:]
        if ac[0] <= 0:
            continue
        ac = ac / ac[0]
        k = lo + int(np.argmax(ac[lo:hi]))
        if ac[k] > 0.45:
            f0.append(fs / k)
    if len(f0) < 5:
        return {"f0_median_hz": None}
    f0 = np.array(f0); st = 12 * np.log2(f0 / np.median(f0))
    return {"f0_median_hz": round(float(np.median(f0)), 1), "f0_range_st": round(float(np.percentile(st, 95) - np.percentile(st, 5)), 2),
            "f0_sd_st": round(float(st.std()), 2), "voiced_share": round(len(f0) / max(frames, 1), 2)}

def measure(x, sr=SR):
    gaps = pauses(x, sr); dur = len(x) / sr; sil = sum(b - a for a, b in gaps)
    return {"duration_s": round(dur, 3), "speech_s": round(dur - sil, 3), "pauses": gaps,
            "peak_dbfs": round(20 * np.log10(np.abs(x).max() + 1e-12), 2), "rms_dbfs": round(10 * np.log10(np.mean(x ** 2) + 1e-12), 2),
            "clipped_samples": int(np.sum(np.abs(x) >= 0.999)), **f0_stats(x, sr)}

# ---------------------------------------------------------------- recognition round trip
_asr = {}
def asr(x, lang, model_name=None, sr=SR):
    """-> (text, mean log-probability, [(word, start, end)])."""
    from faster_whisper import WhisperModel
    model_name = model_name or os.environ.get("NF_WHISPER", "large-v3-turbo")
    if model_name not in _asr:
        _asr[model_name] = WhisperModel(model_name, device="cpu", compute_type="int8", cpu_threads=4)
    y = resample_poly(x, 16000, sr).astype(np.float32)
    segs, _ = _asr[model_name].transcribe(y, language=lang, beam_size=5, word_timestamps=True, condition_on_previous_text=False, vad_filter=False)
    segs = list(segs)
    text = "".join(s.text for s in segs).strip()
    lp = float(np.mean([s.avg_logprob for s in segs])) if segs else -9.0
    words = [(w.word.strip(), round(w.start, 3), round(w.end, 3)) for s in segs for w in (s.words or [])]
    return text, round(lp, 3), words

def lev(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]

NUM = {"0": "zero", "1": "one", "2": "two", "3": "three"}
def norm_en(t):
    t = t.lower().replace("d x f", "dxf").replace("d.x.f.", "dxf").replace("d.x.f", "dxf").replace("won't", "wont").replace("re-imagined", "reimagined")
    t = re.sub(r"[^a-z0-9 ]+", " ", t)
    return [NUM.get(w, w) for w in t.split()]

def wer(ref, hyp):
    r, h = norm_en(ref), norm_en(hyp)
    return round(lev(r, h) / max(len(r), 1), 4)

_tagger = None
def ja_kana(text):
    """Katakana pronunciation string (UniDic 'pron'), names and letters spelled out, punctuation removed."""
    global _tagger
    if _tagger is None:
        import fugashi
        _tagger = fugashi.Tagger()
    t = text
    for k, v in (("NEXUS FABRIC", "ネクサスファブリック"), ("Nexus Fabric", "ネクサスファブリック"), ("NEXUSFABRIC", "ネクサスファブリック"), ("DXF", "ディーエックスエフ"), ("dxf", "ディーエックスエフ")):
        t = t.replace(k, v)
    out = ""
    for w in _tagger(t):
        p = getattr(w.feature, "pron", None) or getattr(w.feature, "kana", None) or w.surface
        out += p if p != "*" else w.surface
    out = "".join(chr(ord(c) + 0x60) if "ぁ" <= c <= "ゖ" else c for c in out)          # hiragana -> katakana
    out = re.sub(r"[^ァ-ヺー]", "", out)
    return out.replace("ヲ", "オ").replace("ヂ", "ジ").replace("ヅ", "ズ")

def cer_ja(ref_reading, hyp_text):
    r = re.sub(r"[^ァ-ヺー]", "", "".join(chr(ord(c) + 0x60) if "ぁ" <= c <= "ゖ" else c for c in ref_reading))
    # particles written は/へ in the reading are pronounced ワ/エ; compare on the tagger's pronunciation of both sides
    r2, h = ja_kana(ref_reading), ja_kana(hyp_text)
    fold = lambda s: re.sub(r"ー", "", s.replace("ウ", "ー").replace("オ", "ー").replace("イ", "ー").replace("エ", "ー"))  # long-vowel spelling is not a reading error
    d = min(lev(fold(r), fold(h)), lev(fold(r2), fold(h)))
    return round(d / max(len(fold(r)), 1), 4)
