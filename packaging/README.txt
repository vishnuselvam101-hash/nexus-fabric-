NEXUS FABRIC - README (pilot evaluation build / パイロット評価版)
=====================================================================

PILOT NOTICE
Pilot evaluation build. Not validated.
Every DXF is NOT-RELEASED. An engineer must check every part before cutting.
Not for confidential drawings. Do not run this program on a network (it has no
TLS). Use desktop mode (the commands under STARTING IT): the server then listens
on 127.0.0.1 only. Desktop mode does not encrypt the database; see
HOW YOUR DATA IS PROTECTED below.
- Outputs must not be used for manufacturing.
- No output is released. No job can reach APPROVED in this build.
- The engineering content has not been independently reviewed.
- Report problems as described in PILOT-NOTES.md (sent with the pilot pack).

パイロット評価版です。検証されていません。
すべての DXF は NOT-RELEASED（未リリース）です。切断の前に、技術者がすべての部品を確認してください。
機密図面には使用しないでください。このプログラムをネットワーク上で動かさないでください（TLS がありません）。
デスクトップモード（STARTING IT の手順）を使用してください。このモードではサーバーは 127.0.0.1 のみで待ち受けます。
デスクトップモードでもデータベースは暗号化されません。下記「HOW YOUR DATA IS PROTECTED」を参照してください。
- 出力を製造に使用してはなりません。
- 本ビルドではリリースは行われません。どのジョブも承認済みになりません。
- 工学的な内容は、独立したレビューを受けていません。
- 問題の報告方法は PILOT-NOTES.md（パイロット資料一式に同梱）に従ってください。

All rights reserved. Evaluation use only — terms to be provided by the owner.

WHAT IT IS
NEXUS FABRIC runs a server on this computer and shows it in your web browser.
Your data stays in the data folder described below.

STARTING IT
- Windows, installed: Start menu > NEXUS FABRIC.
- Windows, portable: open a command prompt in this folder and run
      nexus-fabric.exe --desktop
- Linux: run  ./nexus-fabric.sh  in this folder.

Keep nf-pdf2dxf (nf-pdf2dxf.exe on Windows) in the same folder as the
program: it is the separate PDF -> DXF converter the program starts for a
conversion. Without it, PDF conversion is refused; everything else works.

The browser opens the address shown in the console window (normally
http://127.0.0.1:8787/, or another free port if that one is busy). Keep the
console window open while you work. To stop, press Ctrl+C in it or close the
window. The database is closed cleanly when the program stops.

FIRST RUN
The setup page opens with its one-time token already filled in. Enter a user
name for the administrator and a password of at least 10 characters. The token
is used once and is not shown in the console. If the program is started again
later, the setup link changes; setup happens only while no administrator exists.

WHERE YOUR DATA IS
- Windows: %LOCALAPPDATA%\NexusFabric\data
- Linux:   $XDG_DATA_HOME/nexus-fabric (or ~/.local/share/nexus-fabric)

The server listens on 127.0.0.1 only, so only this computer can reach it. A
second copy of the program on the same data folder is refused with a message.

HOW YOUR DATA IS PROTECTED
This program does not encrypt its database or the whole data folder. Stored
files have their own encryption, but that does not make the data on this
computer an encrypted data set. How well the data is protected depends on this
computer and how it is run. Recommended:
- disk encryption (for example BitLocker on Windows, LUKS on Linux);
- an individual account on the computer for each person;
- access to the data folder only for the people who use this program;
- protected backups of the data folder, kept apart from this computer.
Disk encryption protects a lost or stolen computer. It does not protect a copy
of the data folder taken while the disk is unlocked, and it does not replace
encryption by the program itself. A computer account by itself does not
encrypt the database. The first-run screen shows the same information.

UNINSTALLING
- Installed version: Settings > Apps > NEXUS FABRIC, or the "Uninstall NEXUS
  FABRIC" shortcut in the Start menu. The data folder is kept unless you tick
  "Also delete my data".
- Portable version: delete this folder. Your data stays in the data folder
  above until you delete it.

SMARTSCREEN AND CHECKING THE FILES
The files are not code-signed. Windows may show "Windows protected your PC".
Only continue if the file came from the place you expected. Check it against
SHA256SUMS before running it:
- Windows (PowerShell):  Get-FileHash <file> -Algorithm SHA256
- Linux:                 sha256sum -c SHA256SUMS

This program does not update itself.
