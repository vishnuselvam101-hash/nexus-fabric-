# Localisation: English and Japanese

Round 3. The client has two languages: English (the reference) and Japanese. Every string the client owns is in
`web/i18n.js`; the server and the engine send English, which the client translates only where a template matches exactly.

> The Japanese terminology has not been reviewed by a native-speaking fabrication engineer.

## How the catalogue works

- `web/i18n.js` holds two dictionaries, `EN` and `JA`, with the same 815 keys. A key missing in Japanese falls back to
  English and is recorded by `missing()`; the test fails if anything is recorded.
- `t(key, params)` returns plain text, for `textContent` and attributes. `h(key, params)` returns HTML: the text is
  escaped, `**bold**` becomes `<b>` and a backquoted span becomes monospace. Values from users or the server are escaped
  by the caller: user text with `ud()`, identifiers with `mono()`.
- Placeholders are `{name}`. A plural is `[singular, plural]`. The placeholders of each key are the same in both languages.
- A message is a whole sentence with placeholders; no sentence is built from fragments, so Japanese word order is free.
- Engine and server text arrives in English. It is translated only by exact match against four template tables:
  `SENT` (84 sentences), `WHOLE` (66 texts), `REASON` (15) and `SERVER` (54), plus 224 identifier labels in 21 kinds.
  If any sentence matches no template, the whole text is shown in English and tagged "engine output" or "server
  message". Numbers and identifiers in a matched template pass through unchanged. In a Japanese template, an identifier
  written in backquotes is shown in monospace.
- The language is chosen in this order: `?lang=ja` or `?lang=en` for one page load; the saved choice (`localStorage`,
  key `nf-lang`); the browser's first language if it is Japanese; English. The switch in the top bar, and on the sign-in
  screen, redraws the current screen from data already loaded: no request, no reload, typed input kept.

## Adding a string

1. Add the key to `EN` and to `JA` in `web/i18n.js`, with the same placeholders.
2. Render it with `t()` for text or `h()` for HTML. Do not join fragments by hand.
3. In Japanese, write identifiers, file names and hashes in backquotes, so that they show in monospace.
4. Run `node docs/design/tools/i18n_check.mjs`. It must report no missing key, no placeholder mismatch, and no bare
   English identifier in Japanese.
5. Rebuild with `cargo build --release -p nf-server` (the files are embedded in the binary), then run
   `tests/ui_smoke.js`. Its Japanese walk fails on any English word of three or more letters on the screens it visits.

## What is deliberately not translated

| Kept as it is | Why |
|---|---|
| NEXUS FABRIC; AMOEBA (the module's name) | product and module names |
| DXF, PDF, PNG, JPEG, TIFF, STEP, SHA-256; the units mm, deg, rad, MiB | file formats, the hash algorithm and units are the same in every language |
| Identifiers: parameter keys (`header_radius`), curve names (`seam_a`), gate IDs (`G01` to `G20`), check and record names, layer names (`CUT_OUTER`), the engine id, closure hashes | the engine, the DXF files and the stored records use these exact strings; a translation would break the link to them. Shown in monospace |
| File names and DXF content: `NOT-RELEASED`, `NF_NOT_RELEASED` | the release marking stays English, in the file name and in the file |
| User-entered text: project, user and file names, reasons | shown as typed, in an isolated element (`ud()`) |
| Engine and server sentences that match no template | shown in English and tagged; never translated in part |

Where an identifier has a Japanese label, the identifier stays beside the label, so that a reader can match the screen to
the engine output.

## How the tests guard it

- `tests/ui_smoke.js`, section 14b:
  - the switch: no reload, `<html lang>` follows, the choice survives a reload, `?lang=ja` and `?lang=en` each open in
    their language, and the empty route `#/` opens the dashboard;
  - a walk of the visible text on 32 Japanese states: sign-in, dashboard, job list, the five new-calculation forms, a
    computed job with each of its six tabs and a corrected job's changes, a record opened, the correction dialog, the
    keyboard help, projects, a project, the AMOEBA tabs, manufacturing, verification, administration (users, security,
    audit) and account. The walk skips identifiers, hashes, user text and `[lang=en]`, and accepts only these words:
    NEXUS, FABRIC, AMOEBA, DXF, PDF, PNG, JPEG, TIFF, STEP, SHA, rad;
  - after the walk, `missing()` is empty; in Japanese there is no horizontal overflow on the dashboard, the new-calculation
    form or the job view at 1366, 1024 and 768 pixels; and there is no console or page error in either language.
- `docs/design/tools/i18n_check.mjs` checks key parity, placeholder parity and bare English identifiers, without a browser.

## Glossary, with confidence

| English | Japanese | Confidence |
|---|---|---|
| flat pattern | 展開図 | a trade term |
| development (of a formed surface) | 板展開 | a trade term |
| marking side | けがき面 | a trade term |
| header, branch | 母管, 枝管 | a trade term |
| seam | 継目 | our choice: check |
| correction | 訂正 | our choice: check |
| scrutiny obligation | 要精査項目 | our choice: check |
| gate | ゲート | our choice: check |
| review required | 要レビュー | our choice: check |
| not released | 未リリース | our choice: check |
| refused (input) | 入力拒否 | our choice: check |
| inconclusive | 未確定 | our choice: check |

## Files

- `web/i18n.js`: the catalogue, the template tables and the identifier labels.
- `web/app.js`: the views. It uses `t`, `h`, `eng`, `srv`, `named`, `mono`, `ud` and `jaHtml` (backquoted identifiers in
  Japanese text).
- `docs/design/tools/i18n_check.mjs` and `tests/ui_smoke.js` (section 14b): the checks above.
