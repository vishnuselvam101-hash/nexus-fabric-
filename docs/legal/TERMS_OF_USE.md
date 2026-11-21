# NEXUS FABRIC — Terms of Use / 利用規約

**DRAFT — not reviewed by a lawyer.** / **下書き — 弁護士による確認を受けていません。**

This file is a working draft prepared by the development lead for the owner's lawyer (work plan task 4.5,
`docs/legal/LEGAL_READINESS.md` item 2). It is not legal advice and is not in force. Text in square brackets is
for the owner or the lawyer to supply; nothing in brackets may be guessed. Part B (Japanese) is a translation
prepared without native legal review; **a review of the Japanese text by a lawyer admitted in Japan who is a
native speaker is required** before it is used. Until the reviewed terms replace this draft, the software stays
"evaluation use only".

本書は、開発責任者がオーナーの弁護士のために作成した作業用の下書きです。法的助言ではなく、効力を持ちません。
角括弧内はオーナーまたは弁護士が記入する箇所です。第 B 部（日本語）は母語話者による法的確認を経ていない翻訳であり、
使用前に日本の弁護士（日本語母語話者）による確認が必要です。

---

## Part A — English

**Provider:** [COMPANY LEGAL NAME], [ADDRESS] ("we", "us"). **Contact:** [CONTACT].
**Effective date:** [EFFECTIVE DATE]. **Software:** NEXUS FABRIC, version [VERSION] (the "Software").

### 1. What these terms cover
1.1 These terms govern your use of the Software, whether you run it on your own computer (desktop mode), on a
server you operate, or on an evaluation server operated by us at [HOSTED SERVER ADDRESS] (if offered).
1.2 "You" means the organisation that obtained the Software and the individuals it lets use it. The person
accepting these terms confirms that they may bind that organisation.
1.3 The engineering-responsibility notice (`ENGINEERING_RESPONSIBILITY_NOTICE.md`) and the privacy statement
(`PRIVACY_STATEMENT.md`) form part of these terms. [LAWYER: confirm incorporation by reference is effective.]

### 2. Status of the Software
2.1 The Software is a **development build**. It is not validated, and no qualified fabrication engineer has
reviewed its engineering rules or its outputs. Its engineering rules were written with AI assistance and have
not been independently reviewed. No result produced by the Software can reach the APPROVED state; every DXF
file it writes is marked NOT RELEASED.
2.2 The Software does not model material thickness, bend allowance, weld gap or cutting kerf. Flat patterns and
DXF files do not account for them.
2.3 The Software contains no artificial-intelligence component. It runs fully offline: it makes no network
connection to us or to any third party, sends no telemetry, loads nothing from external servers and does not
update itself.
2.4 In desktop mode the Software does not encrypt its database or the whole data folder. Stored files have their
own encryption, but that does not make the data set encrypted. The protection of the data depends on the
computer and how it is set up (see the install guide).

### 3. Licence
3.1 Subject to these terms, we grant you a non-exclusive, non-transferable, revocable licence to install and use
the Software **for evaluation only**, for [EVALUATION PERIOD] / [LICENCE MODEL AND FEES — owner to decide].
3.2 You may not: (a) use any output of the Software for manufacturing, purchasing, quotation to a third party or
any other purpose where a wrong dimension can cause loss or harm; (b) present any output as released,
reviewed or approved; (c) remove or alter the development-build, NOT RELEASED or review-required markings;
(d) sell, sublicense, rent or distribute the Software; (e) reverse-engineer it except as the law expressly
permits; (f) process drawings you are not entitled to use. [LAWYER: check each restriction against the
governing law.]
3.3 Third-party components are licensed under their own terms, listed in `THIRD_PARTY_NOTICES.txt` shipped
with the Software. Nothing in these terms restricts rights you have under those licences.
3.4 All rights not expressly granted are reserved by [COMPANY LEGAL NAME].

### 4. Your responsibilities
4.1 Every result must be checked by a qualified engineer of your organisation before any use, as set out in
the engineering-responsibility notice.
4.2 You are responsible for the computer or server on which you run the Software, its accounts, access rights,
disk encryption and backups, and for keeping the storage key apart from the data.
4.3 You are responsible for the drawings and data you put into the Software and for having the right to use
them, including any confidentiality duties you owe to your customers.
4.4 If you run the Software for other people, you are responsible for their personal data as described in the
privacy statement.

### 5. No warranty
To the extent the governing law allows, the Software is provided "as is" and "as available", without any
warranty, express or implied, including fitness for a particular purpose, accuracy of results, or
uninterrupted or fault-free operation. [LAWYER: wording and enforceability under [GOVERNING LAW]; consumer and
product-liability rules that cannot be excluded.]

### 6. Limitation of liability
6.1 To the extent the governing law allows, we are not liable for loss or damage arising from the use of any
output of the Software without the engineering review required by section 4.1, or from use contrary to
section 3.2.
6.2 [LIABILITY CAP AND EXCLUSIONS — to be written by the lawyer. No amount is proposed here.]
6.3 Nothing in these terms limits liability that cannot be limited by law, including liability for wilful
misconduct or gross negligence. [LAWYER: confirm and complete.]

### 7. Confidentiality and your data
7.1 We do not receive your drawings, results or user data when you run the Software yourself; they stay on your
computer or server. If you use an evaluation server operated by us, the privacy statement describes what we
hold.
7.2 [CONFIDENTIALITY OBLIGATIONS FOR THE HOSTED EVALUATION SERVER — lawyer.] The README of the development
build states that it is not for confidential drawings.

### 8. Updates and support
The Software does not update itself. New versions, if any, are supplied by [DELIVERY CHANNEL]. Support:
[SUPPORT TERMS OR "none"].

### 9. Term and termination
9.1 These terms apply from the effective date until [END OF EVALUATION / TERM].
9.2 Either party may terminate them with [NOTICE PERIOD] notice; we may terminate immediately if you breach
section 3.2.
9.3 On termination you must stop using the Software and delete it. Your own data stays yours; [EXPORT OR
DELETION ARRANGEMENTS — lawyer and owner].

### 10. Export
The Software contains standard cryptography (AES-256-GCM, Argon2, SHA-256). You must comply with the export
and sanctions laws that apply to you. [LAWYER: is any notification needed for distribution outside Japan?]

### 11. Changes to these terms
[PROCEDURE FOR CHANGING THE TERMS — lawyer; the standard-terms rules of the governing law apply.]

### 12. Governing law and courts
These terms are governed by [GOVERNING LAW]. The [COURT] has exclusive jurisdiction for the first instance.

### 13. Language
These terms are made in English and Japanese. [WHICH VERSION PREVAILS — lawyer.]

---

## Part B — 日本語（翻訳下書き。日本の弁護士による日本語のネイティブ確認が必要です）

**提供者：** [会社の正式名称]、[住所]（以下「当社」という）。**連絡先：** [連絡先]。
**発効日：** [発効日]。**対象ソフトウェア：** NEXUS FABRIC バージョン [バージョン]（以下「本ソフトウェア」という）。

### 第1条（適用範囲）
1. 本規約は、利用者が本ソフトウェアを自己のコンピュータ（デスクトップモード）、自己が運用するサーバ、又は当社が
   [ホスト型サーバのアドレス] にて運用する評価用サーバ（提供する場合に限る）のいずれで利用する場合にも適用される。
2. 「利用者」とは、本ソフトウェアを取得した組織及び当該組織が利用を認めた個人をいう。本規約に同意する者は、当該組織を
   拘束する権限を有することを表明する。
3. エンジニアリング責任に関する告知（`ENGINEERING_RESPONSIBILITY_NOTICE.md`）及びプライバシーステートメント
   （`PRIVACY_STATEMENT.md`）は、本規約の一部を構成する。［弁護士確認：引用による組み入れの有効性］

### 第2条（本ソフトウェアの位置付け）
1. 本ソフトウェアは**開発版**である。妥当性確認は行われておらず、資格を有する製缶・板金技術者による技術規則及び出力の
   審査も行われていない。技術規則は AI の支援を受けて作成されたものであり、独立した審査を受けていない。
   本ソフトウェアのいかなる結果も APPROVED（承認済み）の状態には到達せず、出力されるすべての DXF ファイルには
   NOT RELEASED（未リリース）の表示が付される。
2. 本ソフトウェアは、板厚、曲げ伸び（ベンドアローワンス）、溶接ギャップ及び切断代（カーフ）をモデル化していない。
   展開図及び DXF ファイルはこれらを考慮していない。
3. 本ソフトウェアは人工知能の構成要素を含まない。完全にオフラインで動作し、当社又は第三者へのネットワーク接続、
   テレメトリの送信、外部サーバからの読み込み及び自動更新を一切行わない。
4. デスクトップモードにおいて、本ソフトウェアはデータベース及びデータフォルダ全体を暗号化しない。保存ファイルには個別の
   暗号化が施されているが、これによりデータ全体が暗号化されるものではない。データの保護は、コンピュータ及びその設定に
   依存する（インストールガイドを参照）。

### 第3条（使用許諾）
1. 当社は、本規約に従うことを条件として、利用者に対し、本ソフトウェアを**評価目的に限り**インストールし使用する
   非独占的、譲渡不能かつ撤回可能な権利を、[評価期間]／[ライセンス形態及び料金（オーナーが決定）] の範囲で許諾する。
2. 利用者は、次の行為をしてはならない。(a) 本ソフトウェアの出力を、製造、購買、第三者への見積り、その他寸法の誤りにより
   損失又は危害が生じうる目的に使用すること、(b) 出力をリリース済み、審査済み又は承認済みのものとして示すこと、
   (c) 開発版、NOT RELEASED 又は要審査の表示を削除又は改変すること、(d) 本ソフトウェアを販売、再許諾、貸与又は頒布
   すること、(e) 法令が明示的に認める場合を除き、リバースエンジニアリングを行うこと、(f) 利用権限のない図面を処理すること。
   ［弁護士確認：準拠法に照らした各制限の有効性］
3. 第三者のコンポーネントは、本ソフトウェアに同梱される `THIRD_PARTY_NOTICES.txt` に記載のそれぞれのライセンスに
   従う。本規約は、当該ライセンスに基づく利用者の権利を制限しない。
4. 明示的に許諾されていない一切の権利は、[会社の正式名称] に留保される。

### 第4条（利用者の責任）
1. すべての結果は、いかなる使用の前にも、エンジニアリング責任に関する告知に従い、利用者の組織に属する資格を有する
   技術者が確認しなければならない。
2. 利用者は、本ソフトウェアを稼働させるコンピュータ又はサーバ、そのアカウント、アクセス権、ディスク暗号化及び
   バックアップについて、並びに保存鍵をデータと分けて保管することについて責任を負う。
3. 利用者は、本ソフトウェアに入力する図面及びデータ、その利用権限並びに顧客に対する守秘義務について責任を負う。
4. 利用者が他者のために本ソフトウェアを運用する場合、プライバシーステートメントに記載のとおり、当該他者の個人情報に
   ついて責任を負う。

### 第5条（保証の否認）
準拠法が許す範囲において、本ソフトウェアは「現状有姿」かつ「提供可能な範囲」で提供され、特定目的への適合性、結果の
正確性、中断又は不具合のない動作を含め、明示又は黙示を問わず一切の保証を行わない。
［弁護士確認：[準拠法] の下での文言及び有効性。消費者契約法・製造物責任法等により排除できない責任］

### 第6条（責任の制限）
1. 準拠法が許す範囲において、当社は、第4条第1項の技術的確認を経ずに本ソフトウェアの出力を使用したこと、又は
   第3条第2項に反する使用により生じた損失又は損害について責任を負わない。
2. [責任の上限及び除外事項（弁護士が作成。ここでは金額を提案しない）]
3. 本規約は、故意又は重大な過失による責任その他法令上制限できない責任を制限しない。［弁護士確認・補完］

### 第7条（秘密保持及び利用者のデータ）
1. 利用者が自ら本ソフトウェアを運用する場合、当社は図面、結果又は利用者データを受け取らない。これらは利用者の
   コンピュータ又はサーバに留まる。当社が運用する評価用サーバを利用する場合に当社が保有する情報は、プライバシー
   ステートメントに記載する。
2. [ホスト型評価サーバに関する秘密保持義務（弁護士）] なお、開発版の README には、機密図面には使用しないよう
   記載されている。

### 第8条（更新及びサポート）
本ソフトウェアは自動更新を行わない。新しいバージョンがある場合は、[提供方法] により提供する。サポート：
[サポート条件又は「なし」]。

### 第9条（期間及び終了）
1. 本規約は、発効日から [評価終了日／契約期間] まで適用される。
2. 各当事者は、[予告期間] の予告をもって本規約を終了できる。利用者が第3条第2項に違反した場合、当社は直ちに終了できる。
3. 終了後、利用者は本ソフトウェアの使用を中止し、これを削除しなければならない。利用者自身のデータは利用者に帰属する。
   [データの書き出し又は削除の取扱い（弁護士及びオーナー）]

### 第10条（輸出）
本ソフトウェアは標準的な暗号技術（AES-256-GCM、Argon2、SHA-256）を含む。利用者は、自己に適用される輸出管理及び
制裁関連法令を遵守しなければならない。［弁護士確認：日本国外への頒布に届出等が必要か］

### 第11条（本規約の変更）
[変更手続（弁護士）。民法の定型約款に関する規定等を踏まえること]

### 第12条（準拠法及び管轄）
本規約は [準拠法] に準拠する。[裁判所] を第一審の専属的合意管轄裁判所とする。

### 第13条（言語）
本規約は英語及び日本語で作成する。[優先する言語（弁護士）]

---

## Questions for the lawyer / 弁護士への質問

1. Commercial model: evaluation only, paid licence or subscription? The terms depend on it (owner decision first).
   ／ 商用形態（評価のみ、有償ライセンス、サブスクリプション）。オーナーの決定が先です。
2. Governing law, court, which language prevails, and whether these terms are 定型約款 under the Civil Code
   (and how changes are made). ／ 準拠法・管轄・優先言語、民法上の定型約款に当たるか、変更手続。
3. How far sections 5 and 6 hold under Japanese law (Consumer Contract Act, Product Liability Act), given that
   the Software still writes a DXF file even though it is marked NOT RELEASED. Can a contract clause carry the
   "review by a qualified engineer before any use" requirement? ／ DXF が NOT RELEASED 表示付きでも出力される
   ことを踏まえ、第5条・第6条が日本法上どこまで有効か。「使用前の技術者確認」を契約上の義務にできるか。
4. Liability cap: amount and form (none is proposed here). ／ 責任上限の額と形式（本書では提案していません）。
5. Whether incorporating the responsibility notice and privacy statement by reference is effective, and how
   acceptance is recorded (the Software has no acceptance screen today). ／ 告知・プライバシーステートメントの
   組み入れの有効性と、同意の記録方法（現在、同意画面はありません）。
6. Ownership and registrability of code written with AI assistance, and how to state it to customers
   (LEGAL_READINESS item 10). ／ AI 支援で作成したコードの権利帰属と顧客への説明方法。
7. Export control: is any notification needed for the cryptography when distributing outside Japan?
   ／ 日本国外への頒布時の暗号に関する届出の要否。
8. Confidentiality terms for the hosted evaluation server, and whether customers' drawings may be put on it at
   all. ／ ホスト型評価サーバの秘密保持条項と、顧客図面の取扱い可否。
9. Restriction 3.2(e) (reverse engineering): enforceability under the governing law.
   ／ リバースエンジニアリング禁止条項の有効性。
