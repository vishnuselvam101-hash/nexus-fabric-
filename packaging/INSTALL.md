# NEXUS FABRIC - install guide / インストールガイド

> Development build. Not validated. Outputs must not be used for manufacturing.
> 開発ビルドです。検証されていません。出力を製造に使用してはなりません。

> All rights reserved. Evaluation use only — terms to be provided by the owner.
> 全著作権所有。評価目的のみの使用。利用条件は所有者から提供される予定です（未定）。

---

## English

### Which file to download

| File | For | What it is |
|---|---|---|
| `nexus-fabric-<version>-windows-x64-setup.exe` | Windows | Installer. No administrator rights needed. Installs to `%LOCALAPPDATA%\Programs\NexusFabric`. |
| `nexus-fabric-<version>-windows-x64-portable.zip` | Windows | No installer. Unzip anywhere and run `nexus-fabric.exe --desktop`. |
| `nexus-fabric-<version>-linux-x86_64.tar.gz` | Linux (x86-64) | Program, launcher script and desktop entry. |
| `SHA256SUMS` | everyone | Checksums. Check your download against it first. |

### Check the download

- Linux: `sha256sum -c SHA256SUMS`
- Windows (PowerShell): `Get-FileHash <file> -Algorithm SHA256` and compare with `SHA256SUMS`.
- Signature of `SHA256SUMS` (needs OpenSSL 1.1.1 or later, on Linux or Windows): get `SHA256SUMS.sig` and the public key `nf-release-public.pem` (an Ed25519 public key in PEM format, beginning `-----BEGIN PUBLIC KEY-----`) from the same page as the download. Then run:
  `openssl pkeyutl -verify -pubin -inkey nf-release-public.pem -rawin -in SHA256SUMS -sigfile SHA256SUMS.sig`
  It prints `Signature Verified Successfully` only if `SHA256SUMS` was made by NEXUS FABRIC's release key. Any other answer: do not use the files. Then run the SHA-256 check above on the packages.
  (`packaging/verify_sums.sh <folder> nf-release-public.pem` runs the same check from the source tree.)
  Key fingerprint (SHA-256 of the public key in DER form; compare your copy of the key with it before you trust the signature): `openssl pkey -pubin -in nf-release-public.pem -outform DER | sha256sum`. The fingerprint of the release key is printed by `packaging/sign_sums.sh` when a release is signed and is published with the release. The same key is kept in the source repository at `packaging/nf-release-public.pem`. **Not yet issued:** the repository file is a placeholder until the founder generates the real key; until then the check above fails, and that is the correct result.

### Windows: install

1. Run `nexus-fabric-<version>-windows-x64-setup.exe`. Windows may show "Windows protected your PC" because the file is not code-signed: click *More info*, then *Run anyway*, only if you trust where you got it.
2. Keep the default folder, or choose another. Tick *Desktop shortcut* if you want one.
3. Finish. Start **NEXUS FABRIC** from the Start menu.

### Windows: portable

Unzip the folder, open a command prompt in it, and run `nexus-fabric.exe --desktop`.

### Linux

```sh
tar -xzf nexus-fabric-<version>-linux-x86_64.tar.gz
cd nexus-fabric-<version>
./nexus-fabric.sh
```

To add a launcher to your menu, copy `nexus-fabric.desktop` to `~/.local/share/applications/` and change its `Exec=` line to the full path of `nexus-fabric.sh`.

### Using it

- The console window shows the address (normally `http://127.0.0.1:8787/`). Your browser opens on the first-run page.
- **First run:** the setup token is already filled in. Enter an administrator user name and a password of at least 10 characters.
- The server listens on this computer only (127.0.0.1). Nobody else on the network can reach it.
- A second copy on the same data folder is refused.
- To stop: press Ctrl+C in the console window, or close it.

### Where your data is

- Windows: `%LOCALAPPDATA%\NexusFabric\data`
- Linux: `$XDG_DATA_HOME/nexus-fabric` (or `~/.local/share/nexus-fabric`)

### How your data is protected

- This program does not encrypt its database or the whole data folder.
- Stored files have their own encryption, but that does not make the data on this computer an encrypted data set.
- How well the data is protected depends on this computer and how it is run. Recommended:
  - disk encryption (for example BitLocker on Windows, LUKS on Linux);
  - an individual account on the computer for each person;
  - access to the data folder only for the people who use this program;
  - protected backups of the data folder, kept apart from this computer.
- Disk encryption protects a lost or stolen computer. It does not protect a copy of the data folder taken while the
  disk is unlocked, and it does not replace encryption by the program itself.
- A computer account by itself does not encrypt the database.

The first-run screen in the browser shows the same information.

### Uninstall

- Windows installer: *Settings > Apps > NEXUS FABRIC*, or the *Uninstall NEXUS FABRIC* Start-menu shortcut. Your data is kept unless you tick **Also delete my data**.
- Portable: delete the folder. Your data stays in the data folder above until you delete it.
- Linux: delete the extracted folder and, if you want, the data folder.

---

## 日本語

### ダウンロードするファイル

| ファイル | 対象 | 内容 |
|---|---|---|
| `nexus-fabric-<version>-windows-x64-setup.exe` | Windows | インストーラー。管理者権限は不要です。`%LOCALAPPDATA%\Programs\NexusFabric` にインストールされます。 |
| `nexus-fabric-<version>-windows-x64-portable.zip` | Windows | インストーラーなし。好きな場所に展開し、`nexus-fabric.exe --desktop` を実行します。 |
| `nexus-fabric-<version>-linux-x86_64.tar.gz` | Linux（x86-64） | プログラム、起動スクリプト、デスクトップ項目。 |
| `SHA256SUMS` | 全員 | チェックサム。ダウンロードしたファイルを最初にこれと照合してください。 |

### ダウンロードの確認

- Linux: `sha256sum -c SHA256SUMS`
- Windows（PowerShell）: `Get-FileHash <ファイル> -Algorithm SHA256` を実行し、`SHA256SUMS` と比べます。
- `SHA256SUMS` の署名確認（OpenSSL 1.1.1 以降が必要。Linux でも Windows でも可）: `SHA256SUMS.sig` と公開鍵 `nf-release-public.pem`（PEM 形式の Ed25519 公開鍵。先頭は `-----BEGIN PUBLIC KEY-----`）を、ダウンロードしたページと同じ場所から取得し、次を実行します。
  `openssl pkeyutl -verify -pubin -inkey nf-release-public.pem -rawin -in SHA256SUMS -sigfile SHA256SUMS.sig`
  `Signature Verified Successfully` と表示された場合だけ、NEXUS FABRIC のリリース鍵で作られた `SHA256SUMS` です。それ以外の結果ならファイルを使わないでください。そのうえで上の SHA-256 確認をパッケージごとに行います。
  （ソースツリーからは `packaging/verify_sums.sh <フォルダ> nf-release-public.pem` で同じ確認ができます。）
  鍵のフィンガープリント（公開鍵の DER 形式の SHA-256。信頼する前に、取得した鍵をこれと照合してください）: `openssl pkey -pubin -in nf-release-public.pem -outform DER | sha256sum`。リリース鍵のフィンガープリントは、リリース時に `packaging/sign_sums.sh` が表示し、リリースと一緒に公開されます。同じ鍵はソースリポジトリの `packaging/nf-release-public.pem` にもあります。**未発行:** リポジトリのファイルは、創業者が本番の鍵を生成するまでプレースホルダーです。それまでは上の確認は失敗します。これは正しい結果です。

### Windows: インストール

1. `nexus-fabric-<version>-windows-x64-setup.exe` を実行します。ファイルはコード署名されていないため、「Windows によって PC が保護されました」と表示されることがあります。入手元を信頼できる場合のみ、*詳細情報* → *実行* を選んでください。
2. 既定のフォルダーのままか、別のフォルダーを選びます。デスクトップにショートカットが欲しい場合は *Desktop shortcut* にチェックを入れます。
3. 完了後、スタートメニューから **NEXUS FABRIC** を起動します。

### Windows: ポータブル版

zip を展開し、そのフォルダーでコマンドプロンプトを開いて `nexus-fabric.exe --desktop` を実行します。

### Linux

```sh
tar -xzf nexus-fabric-<version>-linux-x86_64.tar.gz
cd nexus-fabric-<version>
./nexus-fabric.sh
```

メニューに登録するには、`nexus-fabric.desktop` を `~/.local/share/applications/` にコピーし、`Exec=` の行を `nexus-fabric.sh` の完全なパスに書き換えます。

### 使い方

- コンソール画面に アドレス（通常 `http://127.0.0.1:8787/`）が表示され、ブラウザーが最初の設定画面を開きます。
- **初回起動:** 設定トークンは入力済みです。管理者のユーザー名と、10文字以上のパスワードを入力してください。
- サーバーはこのコンピューター（127.0.0.1）だけで待ち受けます。ネットワーク上の他の人からは接続できません。
- 同じデータフォルダーで2つ目のコピーを起動すると拒否されます。
- 停止するには、コンソール画面で Ctrl+C を押すか、ウィンドウを閉じます。

### データの保存場所

- Windows: `%LOCALAPPDATA%\NexusFabric\data`
- Linux: `$XDG_DATA_HOME/nexus-fabric`（または `~/.local/share/nexus-fabric`）

### データの保護について

- このプログラムは、データベースもデータフォルダー全体も暗号化しません。
- 保存されたファイルには個別の暗号化がありますが、それによってこのコンピューター上のデータ全体が暗号化されるわけではありません。
- データがどの程度保護されるかは、このコンピューターとその運用環境によって決まります。推奨事項:
  - ディスクの暗号化（例: Windows の BitLocker、Linux の LUKS）
  - 利用者ごとに、このコンピューターの個別アカウントを使うこと
  - データフォルダーへのアクセスを、このプログラムの利用者だけに限定すること
  - データフォルダーの保護されたバックアップを、このコンピューターとは別の場所に保管すること
- ディスクの暗号化は、紛失・盗難にあったコンピューターを保護します。ディスクのロックが解除されている間に取られたデータフォルダーのコピーは保護せず、プログラム自体による暗号化の代わりにもなりません。
- コンピューターのアカウントだけでは、データベースは暗号化されません。

ブラウザーの初回設定画面にも同じ内容が表示されます。

### アンインストール

- Windows インストーラー版: *設定 > アプリ > NEXUS FABRIC*、またはスタートメニューの *Uninstall NEXUS FABRIC*。*Also delete my data* にチェックを入れない限り、データは残ります。
- ポータブル版: フォルダーを削除します。データは上記のデータフォルダーに残ります。
- Linux: 展開したフォルダーを削除し、必要であればデータフォルダーも削除します。
