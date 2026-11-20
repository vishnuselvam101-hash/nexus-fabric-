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

### アンインストール

- Windows インストーラー版: *設定 > アプリ > NEXUS FABRIC*、またはスタートメニューの *Uninstall NEXUS FABRIC*。*Also delete my data* にチェックを入れない限り、データは残ります。
- ポータブル版: フォルダーを削除します。データは上記のデータフォルダーに残ります。
- Linux: 展開したフォルダーを削除し、必要であればデータフォルダーも削除します。
