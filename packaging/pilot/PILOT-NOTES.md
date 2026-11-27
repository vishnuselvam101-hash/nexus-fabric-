# NEXUS FABRIC pilot evaluation build - notes / パイロット評価版メモ

> Pilot evaluation build. Not validated. Every DXF is NOT-RELEASED; an engineer must check every part before cutting.
> パイロット評価版です。検証されていません。すべての DXF は NOT-RELEASED です。切断前に技術者がすべての部品を確認してください。
> Not for confidential drawings. Do not run on a network (no TLS). Use desktop mode.
> 機密図面には使用しないでください。ネットワーク上で動かさないでください（TLS なし）。デスクトップモードを使用してください。

Source of the limits below: `docs/RELEASE_READINESS.md` and `docs/qa/*` (the repository at the pilot build commit). The version is in the file name of the package.

## 1. What to test

Use desktop mode only (`nexus-fabric.exe --desktop` on Windows; `./nexus-fabric.sh` on Linux). Server mode is not part of this pilot.

1. **First run.** Start the program. The browser opens the setup page with the token filled in. Create the administrator (password at least 10 characters). Check that the setup page shows the data-protection text.
2. **PDF drop to DXF.** Drop a vector PDF page on the convert screen and enter the drawing scale (1:N). Check the conversion result and the DXF and report downloads. The converter reads one page and its vector paths and text; it does not interpret dimensions and does not guess a scale. Keep the separate converter `nf-pdf2dxf` in the program folder (Windows: `nf-pdf2dxf.exe`); without it PDF conversion is refused.
3. **Calculations.** Enter dimensions for one part family on the new-calculation screen. Check how refused values are shown. Compare the calculated values with your own hand calculation and record any difference. A calculated value is not a checked value.
4. **AMOEBA screens.** Check the job's gate results, obligations and withheld parts. Check that the Export DXF control is locked for a REVIEW REQUIRED job, with the reason shown. Check that no control approves or releases a result.
5. **DXF content.** Open a DXF in your CAD or CAM tool. Check the NOT RELEASED marking (annotation layer), the units line and the cut geometry. Record which tool you used and whether it accepts the file.
6. **Desktop mode.** Stop the program with Ctrl+C or by closing the console window. Restart it. Check that the data is still there and that a second copy on the same data folder is refused.
7. **Japanese screens.** Switch the language and check the text. Record any English text that appears in Japanese mode.

## 2. How to report

Send one report per problem, by the channel the founder gives you:

- What you did (numbered steps), what you expected, and what happened.
- Your job id (the number on the job page), and the part family and part you used.
- The program version (from the package file name) and your operating system.
- A screenshot of the screen, and the console text if the program showed an error.
- For a DXF problem: the DXF file name, the CAD or CAM tool, and what the tool reported.

Do not send confidential drawings, customer names or passwords. Use an invented or public example drawing instead. Do not send the data folder.

## 3. Engineer check of every part

Every DXF from this build is NOT-RELEASED. Before any part is cut, a qualified engineer must check every dimension, the reference surfaces, the allowances and the process parameters for that part. The program does not check these for you and has no release control.

## 4. Known limits

Engineering (`docs/RELEASE_READINESS.md`, section "Engineering gaps", and the verdict):

- Not ready for real engineering work. Suitable for internal development, demonstration with non-confidential data, and a supervised pilot evaluation by a factory for review, with no production cutting and no confidential drawings.
- No job can reach APPROVED in this build. No engineer approval is implemented.
- 0 of 56 dependency statements have been reviewed. No mathematical model has been independently reviewed. No process parameter is calibrated.
- Missing calibration forces REVIEW REQUIRED.
- No drawing interpretation. Parameters are typed in and are not linked to any source.
- Reference-surface radii must be supplied by the user. Thickness and allowances are not modelled. Error figures are estimates.
- Four part families only, with several refused sub-cases.
- DXF acceptance by CAM is untested. Units are not declared machine-readably (the units are stated as text, not as `$INSUNITS`).
- No kerf, no bend lines, no part marking, no inner profiles, no plate division.
- Self-consistency audits A1-A4 are not implemented. Replay is a separate process on the same machine and build.
- The gate G18 (no unresolved mandatory scrutiny obligations) never passes, because the rule table is provisional and unreviewed.

Security (`docs/RELEASE_READINESS.md`, section "Security gaps"):

- No TLS in the server (V-20). Use desktop mode on one computer only.
- The database is not encrypted at rest (V-21). Project names, file names, parameters, results and the audit log are readable from the file.
- One key for all stored files, with no rotation and no key service (V-22).
- No multi-factor sign-in, no recovery, no single sign-on (V-23).
- The audit chain is not anchored externally and the hashes are not signed (V-25, V-28).
- The PDF converter is a separate worker (V-26). On Linux it has a parent-death kill, resource limits, Landlock where the kernel has it, and a seccomp allow-list. On Windows it has a Job Object only (memory, CPU time, one process) and no file-system or network isolation. This is a self-assessment, with no independent review.
- No independent security review or penetration test. The security findings are a self-assessment.
- The Windows installer and portable zip were built on Linux and have not been installed or run on Windows before this pilot. Please report any install, start or uninstall problem with the Windows version and screenshots. (Windows 版は Linux 上で作成したもので、本パイロット前に Windows 上でのインストール・起動は未確認です。問題があれば画面写真を添えてご報告ください。)
- No backup or restore tooling. No alerting.

Behaviour found in the bug hunts (`docs/qa/BUG_HUNT_R7.md`, `docs/qa/BUG_HUNT_R8.md`):

- Desktop mode keeps `storage.key` in the data folder, next to the database. Not changed (BH-017, R7).
- The engine and server texts stay in English in Japanese mode on the job, admin security and admin audit pages (BH-R8-007).
- Engine, gate and AMOEBA messages are English on Japanese screens; they are not translated (エンジン、ゲート、AMOEBA のメッセージは、日本語の画面でも英語のまま表示されます).
- A conversion page with an unknown id shows its title twice (BH-R8-008, low).
- Two joints with different declarations can give byte-identical geometry; the declaration appears as text in the DXF only. Whether that is correct is an open engineering question for the owner (BH-004, R7; fixed as a statement only).
- The form accepts some values that the engine then refuses. The refused job is still recorded (BH-014, R7, partly fixed).
- The legacy browser test `tests/ui_smoke.js` is stale (BH-R8-009). Not a product defect.

Not tested in the bug hunts: the path where a job reaches APPROVED with all gates PASS (`docs/qa/BUG_HUNT_R8.md`, section 4).

## 5. Document version

Source: `docs/RELEASE_READINESS.md`, updated 26 November 2026.
