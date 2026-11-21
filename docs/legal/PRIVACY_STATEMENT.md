# NEXUS FABRIC — Privacy Statement / プライバシーステートメント

**DRAFT — not reviewed by a lawyer.** / **下書き — 弁護士による確認を受けていません。**

Working draft for the owner's lawyer (work plan task 4.5, `docs/legal/LEGAL_READINESS.md` item 8). Not legal
advice; not in force. Every statement about what the Software stores was checked against the source code of the
development build (`crates/nf-server`, `web/`, `deploy/`) on branch `work/legal-drafts` (base `2bf369a`); it
must be re-checked against the release commit before publication. Text in square brackets is for the owner or
the lawyer. **The Japanese text (Part B) needs review by a native-speaking lawyer admitted in Japan.**

オーナーの弁護士のための作業用下書きです。法的助言ではなく、効力を持ちません。保存内容に関する記述は開発版の
ソースコードと照合済みですが、公開前にリリース版で再確認が必要です。第 B 部（日本語）は日本の弁護士
（日本語母語話者）による確認が必要です。

---

## Part A — English

**Controller / operator:** see section 1. **Contact:** [CONTACT]. **Effective date:** [EFFECTIVE DATE].

### 1. Two ways the Software is run
**(a) Run by your organisation** — on a computer in desktop mode, or on a server your organisation operates.
All data stays on that computer or server. The Software makes no network connection to [COMPANY LEGAL NAME]
or to anyone else, sends no telemetry or usage statistics, loads nothing from external servers and does not
update itself. [COMPANY LEGAL NAME] receives nothing and holds nothing. Your organisation decides what is
stored and is responsible for it under the law that applies to it.

**(b) Evaluation server operated by [COMPANY LEGAL NAME]** at [HOSTED SERVER ADDRESS] (only if offered). Here
[COMPANY LEGAL NAME], [ADDRESS], holds the data listed below as the business handling it. [LAWYER: obligations
under the Act on the Protection of Personal Information.]

The Software contains no artificial-intelligence component; no data is sent to any AI service.

### 2. What the Software stores
The Software stores the following in its own data folder (desktop mode, Windows:
`%LOCALAPPDATA%\NexusFabric\data`; other systems: `$XDG_DATA_HOME/nexus-fabric` or
`~/.local/share/nexus-fabric`; on a server: the folder set by the operator).

| Data | Purpose | How long it is kept |
|---|---|---|
| User account: user name, role, password salt and password hash (Argon2), active flag, creation time | Sign-in and permissions | Until the account is removed by other means. The Software can deactivate an account but has no function to delete it. |
| Sign-in session: a hash of the session token, user, expiry | Keep you signed in | 12 hours; expired sessions are removed at the next sign-in and by an hourly clean-up |
| Failed sign-in attempts: user name entered, time, network address of the client | Limit password guessing | Removed after 15 minutes (at the next sign-in attempt and by the hourly clean-up) |
| Audit log: time, user name, action, details | A tamper-evident record of what was done | **Kept indefinitely.** The log is append-only and the Software has no function to delete entries. See below. |
| Projects, uploaded drawings, calculations, records, corrections, lessons: including the name of the user who created each and free text typed by users (project names, descriptions, file names, reasons) | The engineering work itself | Kept indefinitely; records are append-only |
| Uploaded files and generated DXF files | The engineering work itself | Kept indefinitely. File contents are encrypted with AES-256-GCM; their names, sizes and hashes are stored in the database |

**Network addresses in the audit log.** Sign-in, failed sign-in and throttled sign-in events record the client
address ("from …"). In desktop mode this is always the computer itself (127.0.0.1). On a server it is the
address of the connecting computer, or, behind a reverse proxy named in `NF_TRUSTED_PROXY`, the address that
proxy reports. Because the audit log is kept indefinitely, these addresses are kept indefinitely.

**The database is not encrypted.** In desktop mode the Software does not encrypt its database or the whole data
folder; anyone who can read the data folder can read user names, the audit log and project text. Stored files
have their own encryption, but that does not make the data set encrypted. Protect the computer with disk
encryption, individual accounts, restricted access to the data folder and protected backups.

### 3. Cookies and browser storage
- One cookie: the session cookie, used only to keep you signed in. It is HttpOnly and SameSite=Strict and
  expires after 12 hours or at sign-out. There are no advertising, analytics or third-party cookies.
- The browser's local storage keeps display preferences only: language, colour theme, the language of
  engineering terms, and panel layout. They never leave the browser.
- The browser's session storage may keep the visible fields of a form you are filling in (never a password or
  token), for the current tab only, so that the form survives a page reload.

### 4. Sharing
The Software shares nothing with anyone. On the evaluation server, [COMPANY LEGAL NAME] does not share the data
with third parties except [EXCEPTIONS REQUIRED BY LAW — lawyer]. Hosting provider: [HOSTING PROVIDER AND
LOCATION — owner].

### 5. Evaluation server only: logs and backups outside the Software
- The reverse proxy in front of the evaluation server may keep its own access log (the supplied configuration
  writes `/var/log/caddy/nexus-fabric.log`), which typically contains network addresses, times and requested
  addresses. [RETENTION OF THE PROXY LOG — operator to confirm.]
- The supplied backup script keeps the 14 newest daily backups; data removed from the live database can remain
  in a backup until that backup is replaced. [CONFIRM WHETHER THE EVALUATION SERVER USES THIS SCRIPT — operator.]
- Operating-system logs of the service: [RETENTION — operator].

### 6. Your rights
To ask for disclosure, correction, deletion or suspension of use of your personal data:
(a) when your organisation runs the Software, contact your organisation's administrator;
(b) for the evaluation server, contact [CONTACT].
The Software cannot delete a user account or remove entries from the audit log; such a request is handled by
[PROCEDURE — lawyer and owner]. [LAWYER: how this meets the Act on the Protection of Personal Information.]

### 7. Changes
[PROCEDURE FOR CHANGING THIS STATEMENT — lawyer.]

---

## Part B — 日本語（翻訳下書き。日本の弁護士による日本語のネイティブ確認が必要です）

**取扱事業者：** 第1項を参照してください。**お問い合わせ先：** [連絡先]。**発効日：** [発効日]。

### 1. 本ソフトウェアの二つの運用形態
**(a) お客様の組織が運用する場合** — デスクトップモードでコンピュータ上で、又はお客様の組織が運用するサーバ上で
動作する場合です。すべてのデータはそのコンピュータ又はサーバに留まります。本ソフトウェアは [会社の正式名称] その他の
いかなる相手にもネットワーク接続を行わず、テレメトリや利用統計を送信せず、外部サーバから何も読み込まず、自動更新も
行いません。[会社の正式名称] は何も受け取らず、何も保有しません。保存する内容はお客様の組織が決定し、適用される
法令に基づきお客様の組織が責任を負います。

**(b) [会社の正式名称] が [ホスト型サーバのアドレス] で運用する評価用サーバの場合**（提供する場合に限ります）。
この場合、[会社の正式名称]（[住所]）が、以下に記載するデータを取り扱う事業者として保有します。
［弁護士確認：個人情報の保護に関する法律上の義務］

本ソフトウェアは人工知能の構成要素を含まず、いかなる AI サービスにもデータを送信しません。

### 2. 本ソフトウェアが保存する情報
本ソフトウェアは、以下の情報を自身のデータフォルダに保存します（デスクトップモードの場合、Windows では
`%LOCALAPPDATA%\NexusFabric\data`、その他の OS では `$XDG_DATA_HOME/nexus-fabric` 又は
`~/.local/share/nexus-fabric`。サーバの場合は運用者が設定したフォルダ）。

| 情報 | 目的 | 保存期間 |
|---|---|---|
| ユーザーアカウント：ユーザー名、権限、パスワードのソルト及びハッシュ（Argon2）、有効フラグ、作成日時 | サインイン及び権限管理 | 他の手段で削除されるまで。本ソフトウェアはアカウントを無効化できますが、削除する機能はありません。 |
| サインインセッション：セッショントークンのハッシュ、ユーザー、有効期限 | サインイン状態の維持 | 12 時間。期限切れのセッションは次回のサインイン時及び 1 時間ごとの整理処理で削除されます。 |
| サインインの失敗：入力されたユーザー名、日時、接続元のネットワークアドレス | パスワード推測の抑止 | 15 分後に削除されます（次回のサインイン試行時及び 1 時間ごとの整理処理）。 |
| 監査ログ：日時、ユーザー名、操作、詳細 | 改ざんを検知できる操作記録 | **無期限に保存されます。** 追記専用であり、本ソフトウェアには記録を削除する機能がありません（下記参照）。 |
| プロジェクト、アップロードされた図面、計算、記録、修正、教訓：各項目を作成したユーザー名及び利用者が入力した自由記述（プロジェクト名、説明、ファイル名、理由）を含む | 技術作業そのもの | 無期限。記録は追記専用です。 |
| アップロードされたファイル及び生成された DXF ファイル | 技術作業そのもの | 無期限。ファイルの内容は AES-256-GCM で暗号化されます。ファイル名、サイズ及びハッシュはデータベースに保存されます。 |

**監査ログ内のネットワークアドレス：** サインイン、サインイン失敗及びサインイン制限の各イベントには、接続元のアドレス
（「from …」）が記録されます。デスクトップモードでは常にそのコンピュータ自身（127.0.0.1）です。サーバでは接続元
コンピュータのアドレス、又は `NF_TRUSTED_PROXY` に指定されたリバースプロキシの背後では当該プロキシが通知する
アドレスです。監査ログは無期限に保存されるため、これらのアドレスも無期限に保存されます。

**データベースは暗号化されていません。** デスクトップモードにおいて、本ソフトウェアはデータベース及びデータフォルダ
全体を暗号化しません。データフォルダを読み取れる者は、ユーザー名、監査ログ及びプロジェクトの記述を読み取ることが
できます。保存ファイルには個別の暗号化が施されていますが、これによりデータ全体が暗号化されるものではありません。
ディスク暗号化、個別のアカウント、データフォルダへのアクセス制限及び保護されたバックアップにより、コンピュータを
保護してください。

### 3. Cookie 及びブラウザのストレージ
- Cookie は一つだけです。サインイン状態の維持のみに使用するセッション Cookie で、HttpOnly 及び
  SameSite=Strict が設定され、12 時間後又はサインアウト時に失効します。広告、分析又は第三者の Cookie は
  使用しません。
- ブラウザのローカルストレージには、表示設定（表示言語、配色、技術用語の言語、パネル配置）のみを保存します。
  これらがブラウザの外に送信されることはありません。
- ブラウザのセッションストレージには、ページの再読み込みに備えて、入力中のフォームの表示項目（パスワード及び
  トークンを除く）を現在のタブに限り一時的に保存することがあります。

### 4. 第三者提供
本ソフトウェアは、いかなる情報も第三者に提供しません。評価用サーバにおいて、[会社の正式名称] は [法令に基づく例外
（弁護士）] を除き、データを第三者に提供しません。ホスティング事業者：[ホスティング事業者及び所在地（オーナー）]。

### 5. 評価用サーバのみ：本ソフトウェア外のログ及びバックアップ
- 評価用サーバの前段にあるリバースプロキシは、独自のアクセスログを保存することがあります（同梱の設定では
  `/var/log/caddy/nexus-fabric.log` に書き込みます）。通常、ネットワークアドレス、日時及び要求されたアドレスが
  含まれます。[プロキシログの保存期間（運用者が確認）]
- 同梱のバックアップスクリプトは、日次バックアップのうち新しい 14 件を保持します。稼働中のデータベースから削除された
  データは、該当するバックアップが置き換えられるまで残ることがあります。[評価用サーバでこのスクリプトを使用しているか
  （運用者が確認）]
- サービスの OS ログ：[保存期間（運用者）]

### 6. お客様の権利
個人情報の開示、訂正、削除又は利用停止のご請求は、
(a) お客様の組織が本ソフトウェアを運用している場合は、その組織の管理者に、
(b) 評価用サーバについては [連絡先] に
お申し出ください。本ソフトウェアはユーザーアカウントの削除及び監査ログの記録の削除ができないため、これらのご請求は
[手続（弁護士及びオーナー）] により対応します。［弁護士確認：個人情報保護法への適合方法］

### 7. 変更
[本ステートメントの変更手続（弁護士）]

---

## Questions for the lawyer / 弁護士への質問

1. The Software has no function to delete a user account or audit entries, and the audit log keeps user names
   and client network addresses indefinitely by design (tamper evidence). Is that compatible with requests for
   deletion or suspension of use under the Act on the Protection of Personal Information, and what retention
   period or pseudonymisation should be required? ／ アカウント及び監査ログを削除する機能がなく、監査ログは
   ユーザー名と接続元アドレスを無期限に保持します。個人情報保護法上の削除・利用停止請求との整合と、必要な保存期間
   又は仮名化について。
2. Is a network address in the audit log personal information (個人情報 or 個人関連情報) in this context?
   ／ 監査ログ内のネットワークアドレスは、この文脈で個人情報又は個人関連情報に当たるか。
3. For the hosted evaluation server: what the operator must publish (purpose of use, safety-management measures,
   contact point), and whether a separate privacy policy for the operator is needed. ／ 評価用サーバの運用者が
   公表すべき事項（利用目的、安全管理措置、窓口）と、運用者用の別ポリシーの要否。
4. The desktop database is not encrypted, and a copied data folder is inside the threat model (owner decision
   20 Nov 2026). How must this be disclosed, and does it affect the operator's safety-management duty?
   ／ デスクトップ版データベースが暗号化されていないこと（データフォルダの複製は脅威モデルの範囲内）の開示方法と、
   安全管理措置義務への影響。
5. Drawings uploaded by customers may contain names of their staff or customers. Who is responsible in each
   deployment, and is a data-processing agreement (委託契約) needed for the evaluation server?
   ／ 図面に含まれる第三者の氏名等について、運用形態ごとの責任主体と、評価用サーバでの委託契約の要否。
6. Overseas hosting: if the evaluation server is outside Japan, what disclosure on cross-border transfer is
   required? ／ 評価用サーバが国外にある場合の越境移転に関する説明義務。
7. Is the browser storage described in section 3 subject to any notice or consent requirement?
   ／ 第3項のブラウザストレージについて通知・同意が必要か。
