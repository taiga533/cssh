# Windows対応実装計画

## 概要

csshプロジェクトをWindows 10/11に対応させる。ローカルマシンがWindowsの場合でも、Linux/Unixリモートホストに対してSSH経由でコマンド実行が可能にする。

## 実装範囲

- **フェーズ1**: 基盤整備（プラットフォーム抽象化、依存関係整理）
- **フェーズ2**: コア機能（deployment.rs修正、リモートセットアップ）
- **フェーズ3**: テスト整備（E2E PowerShellスクリプト、CI/CD）

**リモートホスト**: Linux/Unixのみをサポート（Windows → Linux接続のみ）

## 実装アプローチ

### アーキテクチャ方針

1. **条件付きコンパイル**: `#[cfg(target_family = "windows")]` / `#[cfg(target_family = "unix")]` を使用
2. **プラットフォーム抽象化**: 共通ユーティリティを `cssh-common::platform` モジュールに集約
3. **コード分離**: プラットフォーム固有のロジックは完全に分離し、インターフェースは統一

### 主要な変更点

- Unix shellコマンド（`test -x`, `mkdir -p`, `chmod`, `grep`）をRust標準ライブラリで置き換え
- バイナリ検索で `.exe` 拡張子を自動処理
- パス操作を `PathBuf` で統一
- テストケースをプラットフォーム別に条件分岐

## ファイル別修正計画

### Phase 1: 基盤整備

#### 1. `cssh-common/src/platform.rs` (新規作成)

**目的**: プラットフォーム抽象化ユーティリティの提供

**主要な機能**:
- `executable_extension() -> &'static str`: プラットフォームに応じた拡張子（""またはWindows用".exe"）
- `find_binary(binary_name: &str) -> Result<PathBuf, CsshError>`: 実行ファイル検索（カレントディレクトリ→PATH）
- `is_executable(path: &Path) -> bool`: 実行可能ファイルの判定
  - Unix: パーミッションチェック（`mode & 0o111`）
  - Windows: `.exe` 拡張子の確認
- `make_executable(path: &Path) -> Result<(), CsshError>`: 実行権限の付与
  - Unix: `chmod +x` 相当
  - Windows: 何もしない（.exeファイルは既に実行可能）

**理由**: バイナリ検索と実行権限処理を全クレートで共有し、プラットフォーム固有の処理を隠蔽。

#### 2. `cssh-common/src/lib.rs` (修正)

`pub mod platform;` を追加してモジュールを公開。

#### 3. `cssh-agent/Cargo.toml` (修正)

**変更内容**: `daemonize = { workspace = true }` を削除

**理由**: Unix専用クレートで、現在未使用。Windows対応の妨げとなる。

#### 4. `cssh-cli/src/ssh.rs` (修正)

**変更箇所**:
- `find_agent_binary()` 関数: `cssh-common::platform::find_binary("cssh-agent")` を使用
- `execute_ssh_with_remote_forward()`: SSH実行ファイル検索も `platform::find_binary("ssh")` に置き換え

**理由**: `.exe` 拡張子の処理を透過的に実装し、バイナリ検索ロジックを一元化。

### Phase 2: デプロイメント移植

#### 5. `cssh-cli/src/remote_setup.rs` (新規作成)

**目的**: リモート環境セットアップスクリプトの生成（プラットフォーム別）

**主要な機能**:
- `generate_setup_script(remote_port: u16) -> String`: リモートセットアップ用のシェルスクリプト生成
  - **Unix**: Bashスクリプト（`mkdir -p ~/.cssh`, `chmod +x`, `~/.bashrc` への環境変数追加）
  - **Windows**: 現在未サポート（リモートはLinux/Unix前提）
- `remote_install_dir() -> &'static str`: インストールディレクトリ（`~/.cssh`）
- `remote_binary_name() -> &'static str`: リモートバイナリ名（`cexec`、Windows対応時は条件分岐）

**理由**: プラットフォーム固有のスクリプト生成を分離し、テスト容易性を向上。

#### 6. `cssh-cli/src/deployment.rs` (大幅修正)

**変更箇所**:

**`check_remote_binary_exists()` (38-49行目)**:
- 現状: `ssh ... "test -x ~/.cssh/cexec"`
- 修正後: `remote_setup::remote_install_dir()` と `remote_setup::remote_binary_name()` を使用してパスを構築

**`create_remote_directory()` (70-84行目)**:
- 現状: `ssh ... "mkdir -p ~/.cssh"`
- 修正後: SSH経由で `mkdir -p` を実行（リモートがUnixなので維持可能）

**`scp_to_remote()` (88-113行目)**:
- 現状: `find_local_binary()` でバイナリ検索
- 修正後: `platform::find_binary("cexec")` を使用

**`set_remote_executable()` (116-132行目)**:
- 現状: `ssh ... "chmod +x ~/.cssh/cexec"`
- 修正後: リモートがUnixの場合はそのまま維持（`chmod +x` は必要）

**`setup_remote_path()` (135-152行目)**:
- 現状: `ssh ... "grep -q ... || echo ... >> ~/.bashrc"`
- 修正後: `remote_setup::generate_setup_script()` で生成したスクリプトを実行

**理由**: Unix shellコマンドへの依存を削減し、プラットフォーム抽象化レイヤーを活用。リモートがLinux/Unixであることを前提にするため、一部のUnixコマンドは維持。

#### 7. `cssh-agent/src/executor.rs` (テスト修正)

**変更箇所**: テストケース内のコマンド実行

**修正例**:
```rust
#[tokio::test]
async fn echo_command_executes_correctly() {
    let request = ExecuteRequest {
        #[cfg(target_family = "unix")]
        args: vec!["echo".to_string(), "hello".to_string()],

        #[cfg(target_family = "windows")]
        args: vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "hello".to_string()],
    };
    // ...
}
```

**対象テスト**:
- `echo_command_executes_correctly`
- `nonzero_exit_code_is_handled_correctly` (`false` → Windows: `cmd /C exit 1`)

**理由**: Windows環境でのテスト実行を可能にし、CI/CDでの自動テストをサポート。

#### 8. `cssh-common/tests/integration_test.rs` (テスト修正)

**変更箇所**: Unixコマンドを使用する統合テストを条件付きコンパイルで分離

**修正アプローチ**:
```rust
#[cfg(target_family = "unix")]
mod unix_tests {
    // 既存のテスト（echo, false コマンド使用）
}

#[cfg(target_family = "windows")]
mod windows_tests {
    // Windows版テスト（cmd /C 使用）
}
```

**理由**: プラットフォーム別のテスト実行を可能にし、両環境で適切なコマンドを使用。

### Phase 3: テスト整備

#### 9. `e2e/run_e2e.ps1` (新規作成)

**目的**: Windows環境でのE2Eテストスクリプト（PowerShell版）

**主要な機能**:
- `cargo build --release --workspace` でビルド
- バイナリを `e2e/` ディレクトリにコピー（`cssh.exe`, `cssh-agent.exe`, `cexec.exe`）
- Docker Compose環境の起動（`docker-compose -f e2e\docker-compose.yml up -d`）
- 既存の5つのテストシナリオをPowerShellで実装:
  1. エージェント起動・通信テスト
  2. リモートバイナリ配置テスト
  3. SSH逆ポートフォワーディング経由のコマンド実行
  4. 終了コード伝搬テスト
  5. stderr伝搬テスト

**理由**: Windows開発者がE2Eテストを実行可能にし、プラットフォーム間の互換性を検証。

#### 10. `.github/workflows/windows-ci.yml` (新規作成)

**目的**: GitHub ActionsでのWindows環境CI/CD

**ジョブ内容**:
- `runs-on: windows-latest`
- Rustツールチェーンのインストール
- `cargo build --workspace`
- `cargo test --workspace`
- `cargo build --release --workspace`
- バイナリアーティファクトの確認（`cssh.exe`, `cssh-agent.exe`, `cexec.exe`）

**理由**: Windows環境でのビルドとテストを自動化し、リグレッションを防止。

#### 11. `README.md` (新規セクション追加)

**追加内容**:
- **Windows Support**: Windows 10/11対応の説明
- **Prerequisites on Windows**: OpenSSH Clientのインストール手順
- **Platform-Specific Notes**: バイナリ名、リモート環境の前提条件
- **Troubleshooting on Windows**: よくあるエラーと解決方法

**理由**: Windowsユーザー向けのドキュメントを提供し、セットアップをサポート。

#### 12. `CLAUDE.md` (新規セクション追加)

**追加内容**:
- **Platform Support**: クロスプラットフォーム開発のガイドライン
- **Cross-Platform Guidelines**: `platform` モジュールの使用、条件付きコンパイル、パス処理
- **Windows-Specific Considerations**: リモートホストの前提条件、PowerShell使用

**理由**: 将来の開発者がプラットフォーム対応コードを適切に書けるようにガイド。

## クリティカルファイル一覧

実装で最も重要なファイル（優先順位順）:

1. **cssh-common/src/platform.rs** (新規): プラットフォーム抽象化の基盤
2. **cssh-cli/src/deployment.rs** (修正): デプロイメントロジックの書き直し
3. **cssh-cli/src/remote_setup.rs** (新規): リモートセットアップスクリプト生成
4. **cssh-cli/src/ssh.rs** (修正): バイナリ検索の統一化
5. **cssh-agent/src/executor.rs** (テスト修正): プラットフォーム別テストケース
6. **cssh-common/tests/integration_test.rs** (テスト修正): 統合テストの条件分岐
7. **e2e/run_e2e.ps1** (新規): Windows E2Eテストスクリプト
8. **.github/workflows/windows-ci.yml** (新規): Windows CI/CD設定

## 検証方法

### ビルド検証（Windows環境）

```powershell
# ワークスペース全体のビルド
cargo build --workspace

# リリースビルド
cargo build --release --workspace

# バイナリ生成確認
dir target\debug\cssh.exe
dir target\debug\cssh-agent.exe
dir target\debug\cexec.exe
```

### テスト実行（Windows環境）

```powershell
# 全テスト実行
cargo test --workspace

# クレート別テスト
cargo test -p cssh-common
cargo test -p cssh-agent
cargo test -p cssh-remote
cargo test -p cssh-cli
```

### E2Eテスト（Windows環境）

```powershell
# Docker Desktop for Windows が必要
.\e2e\run_e2e.ps1
```

### 実環境テスト

1. **Windows → Linux (Docker)**:
   - ローカルWindows PCからDockerコンテナ（Linuxサーバー）への接続
   - SSH鍵認証の確認
   - コマンド実行、終了コード、stderr伝搬のテスト

2. **Windows → Linux (AWS EC2等)**:
   - 実際のLinuxサーバーへの接続テスト
   - ネットワーク越しのリバースポートフォワーディング確認

### テストマトリックス

| ローカルOS | リモートOS | 優先度 | テスト方法 |
|-----------|-----------|--------|-----------|
| Windows | Linux | 高 | Docker Desktop |
| Linux | Linux | 高 | Docker Compose（既存） |
| macOS | Linux | 中 | Docker Desktop |

## 重要な考慮事項

### 1. OpenSSH for Windowsの前提

- Windows 10 1809以降に標準搭載
- インストール確認: `ssh -V`, `scp`
- 未インストール時は親切なエラーメッセージを表示

### 2. リモートホストの前提条件

- リモートはLinux/Unixシステム（現時点）
- Bash、標準Unixコマンド（`mkdir`, `chmod`, `test`）が利用可能
- SSH環境変数設定（`AcceptEnv`, `SetEnv`）が有効

### 3. パス処理の統一

- 常に `std::path::PathBuf` を使用
- 文字列連結でパスを構築しない
- SSH/SCPコマンド引数では `/` を使用（SSH側でパース）

### 4. 環境変数の設定

- Windows: PowerShell Profile (`$PROFILE.CurrentUserAllHosts`) に `$env:CSSH_PORT` を追加
- Linux/Unix: `~/.bashrc` に `export CSSH_PORT=...` を追加
- リモート側でのSSH環境変数受け渡し確認が必要

### 5. プラットフォーム固有のエラーハンドリング

- SSH/SCPコマンドが見つからない場合、プラットフォームに応じたインストール手順を表示
- Windows: Settings > Apps > Optional Features > OpenSSH Client
- Linux/Unix: パッケージマネージャ（`apt`, `yum`等）での `openssh-client` インストール

### 6. 条件付きコンパイルの一貫性

- `#[cfg(target_family = "unix")]` と `#[cfg(target_family = "windows")]` を使用
- `target_os = "linux"` や `target_os = "windows"` より広い条件を優先（macOS対応のため）
- 両方の分岐で同じ関数シグネチャを維持

## 実装後の確認ポイント

- [ ] Windows環境でビルドが成功する
- [ ] すべてのユニットテストがWindows/Linux両方で通る
- [ ] Windows → Linux Docker接続でコマンド実行が成功する
- [ ] E2Eテスト（run_e2e.ps1）がすべて成功する
- [ ] GitHub Actions Windows CIがグリーンになる
- [ ] README.mdにWindows対応セクションが追加されている
- [ ] バイナリ検索で `.exe` 拡張子が自動処理される
- [ ] SSH/SCPコマンドの不在時に親切なエラーメッセージが表示される

## トレードオフと将来的な拡張

### 現在の決定

- **リモートOS**: Linux/Unixのみサポート → 実装がシンプル、最も一般的なユースケース
- **OS検出**: 自動検出なし → 追加のSSH接続が不要、パフォーマンス良好
- **E2Eテスト**: PowerShellスクリプト作成 → Windows開発者の開発体験向上

### 将来的な拡張候補

- Windows → Windows SSH接続のサポート（リモートもWindows）
- リモートOS自動検出機能（`uname` / `ver` コマンド実行）
- クロスコンパイル対応ドキュメント（Linux上でWindows用バイナリをビルド）
- Windows Service化（現在は `daemonize` 削除により未対応）

## まとめ

この実装計画により、csshプロジェクトはWindows 10/11環境でビルド・実行可能になり、Linux/Unixリモートホストへのコマンド実行が可能になります。フェーズ1+2の実装でコア機能が動作し、フェーズ3のテスト整備により継続的な品質保証が実現します。
