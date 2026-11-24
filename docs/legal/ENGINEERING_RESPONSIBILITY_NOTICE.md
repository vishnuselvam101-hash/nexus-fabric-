# NEXUS FABRIC — Engineering Responsibility Notice / エンジニアリング責任に関する告知

**DRAFT — not reviewed by a lawyer.** / **下書き — 弁護士による確認を受けていません。**

Working draft for the owner's lawyer (work plan task 4.5, `docs/legal/LEGAL_READINESS.md` item 3). Not legal
advice; not in force. The facts below follow `docs/project/READINESS.md` and `docs/project/MASTER_GOAL.md` as of
21 Nov 2026 and must be re-checked against the release. Text in square brackets is for the owner or the lawyer.
**The Japanese text (Part B) needs review by a native-speaking lawyer admitted in Japan.**

オーナーの弁護士のための作業用下書きです。法的助言ではなく、効力を持ちません。記載事項は 2026 年 11 月 21 日時点の
社内文書に基づいており、リリース版で再確認が必要です。第 B 部（日本語）は日本の弁護士（日本語母語話者）による確認が
必要です。

---

## Part A — English

**Provider:** [COMPANY LEGAL NAME], [ADDRESS]. **Contact:** [CONTACT]. **Effective date:** [EFFECTIVE DATE].

### 1. What the Software is and is not
NEXUS FABRIC computes flat patterns and writes DXF files from dimensions entered by its user. It is a
**development build**:
- It is not validated. No qualified fabrication engineer has reviewed its engineering rules or its outputs
  (0 of 56 engineering statements reviewed). Its engineering rules are evidence level EV-1 and have
  not been independently reviewed.
- An independent check of the geometry exists and is still in progress; passing the automated software checks
  does not mean the geometry or the fabrication rules are right.
- **No result can reach the APPROVED state.** Engineer approval is not implemented. Every DXF file is marked
  **NOT RELEASED**.
- The Software contains no artificial-intelligence component. It runs fully offline and does not update itself.

### 2. What the Software does not model
The Software does **not** model:
- material thickness;
- bend allowance (and no K-factor is assumed);
- weld gap;
- cutting kerf.

Flat patterns and DXF files are geometric developments only. Allowances for these effects, and any other
process value, must be decided and applied by your qualified engineer. The Software also does not decide
whether a narrow gap at a seam in a flat pattern is acceptable for cutting.

### 3. Your responsibility
3.1 No output of the Software may be used for manufacturing, cutting, purchasing, quotation or any other
purpose where a wrong dimension can cause loss or harm, unless a qualified engineer of your organisation has
checked it independently against the drawing and your own fabrication rules and has taken responsibility for
it.
3.2 A result shown as passing its checks, or offered as a DXF, has passed the Software's own checks only. It is
not a release, a review or an approval.
3.3 When the Software refuses a case or reports a result as inconclusive or review-required, do not work around
it by changing inputs until it passes.
3.4 Do not remove the NOT RELEASED marking from a file name or drawing, and do not pass a file on to a cutting
machine, supplier or customer as if it were released.
3.5 Keep the drawing, the inputs and the record of the engineer's check for each part you make.

### 4. Reporting a problem
If you find a wrong result, please report it to [CONTACT] with the inputs and the output. [WHETHER AND HOW THE
VENDOR RESPONDS — owner and lawyer.]

### 5. Relation to the terms of use
This notice forms part of the terms of use. [LAWYER: effect of this notice as a contract term and as a product
warning.]

---

## Part B — 日本語（翻訳下書き。日本の弁護士による日本語のネイティブ確認が必要です）

**提供者：** [会社の正式名称]、[住所]。**お問い合わせ先：** [連絡先]。**発効日：** [発効日]。

### 1. 本ソフトウェアの位置付け
NEXUS FABRIC は、利用者が入力した寸法から展開図を計算し、DXF ファイルを出力するソフトウェアです。本ソフトウェアは
**開発版**です。
- 妥当性確認は行われていません。資格を有する製缶・板金技術者による技術規則及び出力の審査は行われていません
  （56 項目の技術的記述のうち審査済みは 0 項目）。技術規則は証拠レベル EV-1 で、独立した審査を
  受けていません。
- 形状の独立検証は存在しますが、まだ完了していません。自動ソフトウェア検査に合格したことは、形状や製作上の規則が
  正しいことを意味しません。
- **いかなる結果も APPROVED（承認済み）の状態には到達しません。** 技術者による承認機能は実装されていません。
  すべての DXF ファイルには **NOT RELEASED（未リリース）** の表示が付されます。
- 本ソフトウェアは図面の自動読み取りの構成要素を含みません。完全にオフラインで動作し、自動更新も行いません。

### 2. 本ソフトウェアがモデル化していない事項
本ソフトウェアは、次の事項をモデル化して**いません**。
- 板厚
- 曲げ伸び（ベンドアローワンス）（K ファクターも想定していません）
- 溶接ギャップ
- 切断代（カーフ）

展開図及び DXF ファイルは幾何学的な展開のみを示します。これらの影響に対する補正その他の加工値は、お客様の
資格を有する技術者が決定し、適用してください。また、本ソフトウェアは、展開図の継ぎ目に生じる狭い隙間が切断上
許容できるかどうかを判断しません。

### 3. お客様の責任
1. 本ソフトウェアの出力を、製造、切断、購買、見積りその他寸法の誤りにより損失又は危害が生じうる目的に使用する場合は、
   事前にお客様の組織に属する資格を有する技術者が、図面及びお客様自身の製作基準に照らして独立に確認し、その責任を
   引き受けてください。それ以外の使用はできません。
2. 検査に合格したと表示された結果や DXF として提供された結果は、本ソフトウェア自身の検査に合格したにすぎず、
   リリース、審査又は承認を意味するものではありません。
3. 本ソフトウェアがケースを拒否した場合、又は結果を判定不能もしくは要審査と表示した場合に、合格するまで入力を変更して
   回避しないでください。
4. ファイル名や図面から NOT RELEASED の表示を削除しないでください。また、リリース済みであるかのようにファイルを
   切断機、仕入先又は顧客に渡さないでください。
5. 製作する部品ごとに、図面、入力値及び技術者による確認の記録を保管してください。

### 4. 不具合の報告
誤った結果を発見された場合は、入力値と出力を添えて [連絡先] までご報告ください。[当社の対応の有無及び方法
（オーナー及び弁護士）]

### 5. 利用規約との関係
本告知は利用規約の一部を構成します。［弁護士確認：契約条項及び製品上の警告としての効力］

---

## Questions for the lawyer / 弁護士への質問

1. The Software still produces a DXF file for a case it marks review-required, with NOT RELEASED in the file.
   Is the marking plus this notice an adequate warning under the Product Liability Act (指示・警告上の欠陥), and
   is software even a "product" there in this delivery form? ／ 要審査のケースでも NOT RELEASED 表示付きの DXF を
   出力します。表示と本告知で製造物責任法上の警告として十分か。この提供形態のソフトウェアが「製造物」に当たるか。
2. Can "review by a qualified engineer before any use" be made a binding obligation of the customer organisation,
   and what follows if the customer's staff ignore it? ／ 「使用前の技術者確認」を顧客組織の拘束力ある義務にできるか。
   従業員が無視した場合の帰結。
3. How must the fact that the engineering rules are evidence level EV-1 and are unreviewed be disclosed?
   ／ 技術規則が証拠レベル EV-1 で独立した審査を受けていないことの開示方法。
4. Is "qualified engineer" sufficiently defined, or should the notice name a qualification? (None is named here;
   the owner and a fabrication engineer must decide.) ／ 「資格を有する技術者」の定義は十分か。特定の資格を
   明記すべきか（本書では明記していません）。
5. Where must this notice be shown to be effective (installer, first-run screen, each DXF, the contract)?
   ／ 効力を持たせるために本告知をどこに表示すべきか（インストーラ、初回画面、各 DXF、契約書）。
