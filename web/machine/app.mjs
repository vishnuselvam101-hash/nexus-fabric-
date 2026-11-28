// Machine export panel. Plain ES module, no build step. Server text is shown with textContent only.
const T = {
  en: {
    title: "Machine export", note: "Files are written only for parts that passed their digest and are not held. Nothing starts a machine.",
    calc: "Job", prof: "Machine profile", folder: "Folder", go: "Export to folder", none: "(none)",
    done: "Written", written: "Files written to the folder:", error: "Not written. Nothing was written.",
    lang: "日本語", noFolder: "No export folder is configured on this server.", noProfile: "No machine profile yet. Ask an engineer to create one.",
    codes: {
      held: "A part on this job is held (a check is open or failed). Its file is not written.",
      not_certified: "A part did not pass its AMOEBA digest. Its file is not written.",
      no_certificate: "A part has no digest record. Its file is not written.",
      hash_mismatch: "A file no longer matches its digest record. Nothing was written.",
      engine_stale: "This job was made by an older engine. Recompute it first.",
      part_mismatch: "A digest record belongs to another part. Nothing was written.",
      person_only: "Export is for a person. An agent can only propose it.",
      licence_required: "The licence does not allow export. Nothing was written.",
      unmapped_layer: "A layer has no name in this machine profile. Nothing was written.",
      unencodable: "A character cannot be written in this profile's text encoding. Nothing was written.",
      unsupported_entity: "This profile's DXF version cannot hold one of the shapes. Nothing was written.",
      units_mismatch: "The file is not in millimetres. Nothing was written.",
      exists: "A file with this name is already in the folder. Nothing was overwritten.",
      io: "The folder could not be written. Nothing was kept.",
    },
  },
  ja: {
    title: "機械向け書き出し", note: "digest に合格し保留されていない部品のファイルだけを書き出します。機械は起動しません。",
    calc: "ジョブ", prof: "機械プロファイル", folder: "フォルダ", go: "フォルダへ書き出す", none: "(なし)",
    done: "書き出しました", written: "フォルダに書き出したファイル:", error: "書き出しませんでした。何も書き込んでいません。",
    lang: "English", noFolder: "書き出しフォルダが設定されていません。", noProfile: "機械プロファイルがありません。技術者に作成を依頼してください。",
    codes: {
      held: "このジョブの部品が保留中です（確認が未了または不合格）。ファイルは書き出しません。",
      not_certified: "部品が AMOEBA digest に合格していません。ファイルは書き出しません。",
      no_certificate: "部品の digest 記録がありません。ファイルは書き出しません。",
      hash_mismatch: "ファイルが digest 記録と一致しません。何も書き込んでいません。",
      engine_stale: "このジョブは古いエンジンで作成されました。先に再計算してください。",
      part_mismatch: "digest 記録が別の部品のものです。何も書き込んでいません。",
      person_only: "書き出しは担当者のみ可能です。AI は提案のみできます。",
      licence_required: "ライセンスにより書き出しはできません。何も書き込んでいません。",
      unmapped_layer: "このプロファイルに無いレイヤーがあります。何も書き込んでいません。",
      unencodable: "この文字コードで表せない文字があります。何も書き込んでいません。",
      unsupported_entity: "この DXF 版では表せない形状があります。何も書き込んでいません。",
      units_mismatch: "ファイルの単位がミリメートルではありません。何も書き込んでいません。",
      exists: "同名のファイルがあるため上書きしません。",
      io: "フォルダに書き込めませんでした。",
    },
  },
};
let lang = (() => { try { return localStorage.getItem("nf-lang") === "ja" ? "ja" : "en"; } catch { return "en"; } })();
const $ = (id) => document.getElementById(id);
const H = { "x-nf-request": "1", "content-type": "application/json" };

function paint() {
  const t = T[lang];
  document.documentElement.lang = lang;
  $("title").textContent = t.title;
  $("note").textContent = t.note;
  $("l-calc").textContent = t.calc;
  $("l-prof").textContent = t.prof;
  $("l-folder").textContent = t.folder;
  $("go").textContent = t.go;
  $("lang").textContent = t.lang;
  document.title = t.title;
}

function fill(sel, items, label) {
  sel.replaceChildren();
  for (const it of items) {
    const o = document.createElement("option");
    o.value = it.value;
    o.textContent = it.label;
    sel.append(o);
  }
  if (!items.length) {
    const o = document.createElement("option");
    o.value = "";
    o.textContent = label;
    sel.append(o);
  }
}

async function getJson(url) {
  const r = await fetch(url, { headers: { "x-nf-request": "1" }, credentials: "same-origin" });
  if (!r.ok) throw new Error(String(r.status));
  return r.json();
}

async function load() {
  const [calcs, profs, folders] = await Promise.all([
    getJson("/api/calculations").catch(() => []),
    getJson("/api/machine/profiles").catch(() => []),
    getJson("/api/machine/folders").catch(() => []),
  ]);
  fill($("calc"), (calcs || []).map((c) => ({ value: String(c.id), label: `#${c.id} ${c.state ?? ""}`.trim() })), T[lang].none);
  fill($("prof"), (profs || []).map((p) => ({ value: JSON.stringify({ id: p.id, version: p.version }), label: `${p.id} v${p.version} - ${p.profile?.name ?? ""}` })), T[lang].noProfile);
  fill($("folder"), (folders || []).map((f) => ({ value: String(f.index), label: f.path })), T[lang].noFolder);
  $("go").disabled = !(folders && folders.length && profs && profs.length && calcs && calcs.length);
}

async function exportNow() {
  const t = T[lang];
  const out = $("result");
  out.className = "msg";
  out.textContent = "";
  let prof;
  try { prof = JSON.parse($("prof").value); } catch { return; }
  const body = { profile: prof.id, version: prof.version, folder: Number($("folder").value) };
  $("go").disabled = true;
  try {
    const r = await fetch(`/api/calculations/${encodeURIComponent($("calc").value)}/export`, {
      method: "POST", headers: H, credentials: "same-origin", body: JSON.stringify(body),
    });
    const v = await r.json().catch(() => ({}));
    if (r.ok) {
      out.classList.add("ok");
      out.textContent = `${t.done}. ${t.written}`;
      const ul = document.createElement("ul");
      for (const f of v.files || []) {
        const li = document.createElement("li");
        li.textContent = `${f.file} (${f.part}) sha256 ${String(f.sha256).slice(0, 12)}`;
        ul.append(li);
      }
      out.append(ul);
    } else {
      out.classList.add("bad");
      out.textContent = `${t.error} ${t.codes[v.code] ?? ""}`.trim();
    }
  } catch {
    out.classList.add("bad");
    out.textContent = t.error;
  } finally {
    $("go").disabled = false;
  }
}

$("lang").addEventListener("click", () => {
  lang = lang === "en" ? "ja" : "en";
  try { localStorage.setItem("nf-lang", lang); } catch { /* private window: keep in memory */ }
  paint();
  load();
});
$("go").addEventListener("click", exportNow);
paint();
load();
