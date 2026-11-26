# NEXUS FABRIC pilot evaluation build - install / パイロット評価版のインストール

> Pilot evaluation build. Not validated. Every DXF is NOT-RELEASED; an engineer must check every part before cutting.
> パイロット評価版です。検証されていません。すべての DXF は NOT-RELEASED です。切断前に技術者がすべての部品を確認してください。
> Not for confidential drawings. Do not run on a network (no TLS). Use desktop mode.
> 機密図面には使用しないでください。ネットワーク上で動かさないでください（TLS なし）。デスクトップモードを使用してください。

Files in the pilot pack (`<version>` is the program version):

| File | For |
|---|---|
| `nexus-fabric-<version>-windows-x64-setup.exe` | Windows installer |
| `nexus-fabric-<version>-windows-x64-portable.zip` | Windows, no installer |
| `nexus-fabric-<version>-linux-x86_64.tar.gz` | Linux (x86-64) |
| `SHA256SUMS` | checksums |
| `INSTALL-PILOT.md`, `PILOT-NOTES.md` | this guide and the test notes |

## 1. Check the download (first)

- Linux: `sha256sum -c SHA256SUMS`. Every line must end with `OK`.
- Windows (PowerShell): `Get-FileHash <file> -Algorithm SHA256` and compare the value with the line for that file in `SHA256SUMS`.

Do not install a file whose checksum does not match. Ask the founder for a new copy.

## 2. Windows: installer

1. Run `nexus-fabric-<version>-windows-x64-setup.exe`. The file is not code-signed, so Windows may show "Windows protected your PC". Continue only if the file came from the place you expected: *More info*, then *Run anyway*.
2. No administrator rights are needed. The default folder is `%LOCALAPPDATA%\Programs\NexusFabric`. Tick *Desktop shortcut* if you want one.
3. Start **NEXUS FABRIC** from the Start menu.

## 3. Windows: portable

1. Unzip the folder anywhere.
2. Open a command prompt in that folder and run `nexus-fabric.exe --desktop`.
3. Keep `nf-pdf2dxf.exe` in the same folder. Without it PDF conversion is refused; the rest works.

## 4. Linux

```sh
tar -xzf nexus-fabric-<version>-linux-x86_64.tar.gz
cd nexus-fabric-<version>
./nexus-fabric.sh
```

Keep `nf-pdf2dxf` in the same folder (the program starts it for PDF conversion).
To add a menu launcher, copy `nexus-fabric.desktop` to `~/.local/share/applications/` and change its `Exec=` line to the full path of `nexus-fabric.sh`.

## 5. Start in desktop mode

- Use desktop mode only: `nexus-fabric.exe --desktop` (Windows) or `./nexus-fabric.sh` (Linux, which starts desktop mode).
- The console window shows the address, normally `http://127.0.0.1:8787/` (another free port if 8787 is busy). Your browser opens on the setup page.
- **First run:** the setup token is already filled in. Enter an administrator user name and a password of at least 10 characters.
- The server listens on 127.0.0.1 only. Other computers on the network cannot reach it.
- Keep the console window open while you work. To stop: press Ctrl+C in it, or close it. The database is closed when the program stops.
- A second copy on the same data folder is refused with a message.

## 6. Where your data is

- Windows: `%LOCALAPPDATA%\NexusFabric\data`
- Linux: `$XDG_DATA_HOME/nexus-fabric` (or `~/.local/share/nexus-fabric`)

## 7. How your data is protected

- The database is not encrypted, and the data folder is not encrypted as a whole. Stored files have their own encryption, which does not make the data on the computer an encrypted data set.
- The storage key file `storage.key` is in the data folder, next to the database.
- Recommended: disk encryption (BitLocker on Windows, LUKS on Linux); one account per person; access to the data folder for the users of this program only; protected backups kept apart from the computer.
- Disk encryption does not protect a copy of the data folder taken while the disk is unlocked.

## 8. Uninstall

- Windows installer: *Settings > Apps > NEXUS FABRIC*, or the *Uninstall NEXUS FABRIC* Start-menu shortcut. Your data is kept unless you tick **Also delete my data**.
- Windows portable: delete the folder. Your data stays in the data folder until you delete it.
- Linux: delete the extracted folder and, if you want, the data folder in section 6.

The program does not update itself.

---

## 日本語

### 1. ダウンロードの確認（最初に実施）

- Linux: `sha256sum -c SHA256SUMS`。すべての行が `OK` で終わる必要があります。
- Windows（PowerShell）: `Get-FileHash <ファイル> -Algorithm SHA256` を実行し、`SHA256SUMS` 内の該当ファイルの値と比べます。

値が一致しないファイルはインストールしないでください。作成者に新しいコピーを依頼してください。

### 2. Windows: インストーラー版

1. `nexus-fabric-<version>-windows-x64-setup.exe` を実行します。コード署名されていないため「Windows によって PC が保護されました」と表示されることがあります。入手元を確認できる場合のみ、*詳細情報* → *実行* を選んでください。
2. 管理者権限は不要です。既定のフォルダーは `%LOCALAPPDATA%\Programs\NexusFabric` です。デスクトップのショートカットが必要なら *Desktop shortcut* にチェックを入れます。
3. スタートメニューから **NEXUS FABRIC** を起動します。

### 3. Windows: ポータブル版

1. zip を任意の場所に展開します。
2. そのフォルダーでコマンドプロンプトを開き、`nexus-fabric.exe --desktop` を実行します。
3. `nf-pdf2dxf.exe` を同じフォルダーに置いてください。ない場合、PDF の変換は拒否されます（他の機能は動作します）。

### 4. Linux

```sh
tar -xzf nexus-fabric-<version>-linux-x86_64.tar.gz
cd nexus-fabric-<version>
./nexus-fabric.sh
```

PDF 変換のため、`nf-pdf2dxf` を同じフォルダーに置いてください。

### 5. デスクトップモードで起動

- デスクトップモードのみを使用してください。Windows は `nexus-fabric.exe --desktop`、Linux は `./nexus-fabric.sh`（デスクトップモードで起動します）。
- コンソール画面にアドレス（通常 `http://127.0.0.1:8787/`。8787 が使用中の場合は別の空きポート）が表示され、ブラウザーが初回設定画面を開きます。
- **初回起動:** 設定トークンは入力済みです。管理者のユーザー名と、10文字以上のパスワードを入力してください。
- サーバーは 127.0.0.1 のみで待ち受けます。ネットワーク上の他のコンピューターからは接続できません。
- 作業中はコンソール画面を開いたままにしてください。停止するには、コンソール画面で Ctrl+C を押すか、ウィンドウを閉じます。
- 同じデータフォルダーで2つ目のコピーを起動すると、メッセージが表示され拒否されます。

### 6. データの保存場所

- Windows: `%LOCALAPPDATA%\NexusFabric\data`
- Linux: `$XDG_DATA_HOME/nexus-fabric`（または `~/.local/share/nexus-fabric`）

### 7. データの保護について

- データベースもデータフォルダー全体も暗号化されません。保存されたファイルには個別の暗号化がありますが、それによってこのコンピューター上のデータ全体が暗号化されるわけではありません。
- 保存鍵ファイル `storage.key` は、データベースと同じデータフォルダーにあります。
- 推奨: ディスクの暗号化（Windows は BitLocker、Linux は LUKS）、利用者ごとの個別アカウント、データフォルダーへのアクセスをこのプログラムの利用者に限定すること、データフォルダーの保護されたバックアップをこのコンピューターとは別の場所に保管すること。
- ディスクの暗号化は、ディスクのロックが解除されている間に取られたデータフォルダーのコピーは保護しません。

### 8. アンインストール

- Windows インストーラー版: *設定 > アプリ > NEXUS FABRIC*、またはスタートメニューの *Uninstall NEXUS FABRIC*。*Also delete my data* にチェックを入れない限り、データは残ります。
- Windows ポータブル版: フォルダーを削除します。データは削除するまでデータフォルダーに残ります。
- Linux: 展開したフォルダーを削除し、必要であれば 6. のデータフォルダーも削除します。

本プログラムは自動で更新されません。
