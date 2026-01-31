# GitHub Actions CI/CD Implementation Plan

## Overview
OSS配布のための包括的なGitHub Actionsワークフローを実装します。テスト、lint、クロスプラットフォームビルド、crates.io公開を自動化します。

## User Requirements
- **対象プラットフォーム**: Linux x64, Windows x64
- **crates.io**: 公開する
- **ライセンス**: MIT

## Implementation Details

### 1. CI Workflow (`.github/workflows/ci.yml`)

**トリガー**: `push`と`pull_request`（mainブランチ）

**ジョブ構成**:

#### `format-check`
- `cargo fmt --all -- --check`でコードフォーマット検証
- 実行環境: `ubuntu-latest`
- Rust toolchain: `stable` + `rustfmt` component

#### `lint`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`でlint検証
- 実行環境: `ubuntu-latest`
- Rust toolchain: `stable` + `clippy` component
- キャッシュ: `Swatinem/rust-cache@v2`

#### `test` (matrix)
- ユニット・統合テスト実行
- マトリックス:
  - OS: `ubuntu-latest`, `windows-latest`
  - Rust: `stable`, `1.89.0` (MSRV)
- ステップ:
  1. `cargo build --workspace --verbose`
  2. `cargo test --workspace --verbose`
- キャッシュ: `Swatinem/rust-cache@v2`（OS・Rustバージョン別）

#### `e2e-test`
- Docker環境でのE2Eテスト
- 実行環境: `ubuntu-latest`のみ
- 依存: `test`ジョブの成功
- ステップ:
  1. Docker Composeのセットアップ確認
  2. `bash e2e/run_e2e.sh`実行
  3. 失敗時のログ出力（`if: failure()`）

#### `commit-check` (PRのみ)
- Conventional Commits形式の検証
- 条件: `github.event_name == 'pull_request'`
- `cocogitto/cocogitto-action@v3`を使用

---

### 2. Release Workflow (`.github/workflows/release.yml`)

**トリガー**: タグプッシュ（`v*.*.*`パターン）

**ジョブ構成**:

#### `create-release`
- GitHub Releaseの作成
- 実行環境: `ubuntu-latest`
- ステップ:
  1. タグからバージョン番号を抽出
  2. `softprops/action-gh-release@v2`でドラフトリリース作成
  3. リリースIDを出力

#### `build-binaries` (matrix)
- クロスプラットフォームビルド
- 依存: `create-release`の完了
- マトリックス:
  ```yaml
  matrix:
    include:
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
        artifact_name: cssh-linux-x86_64
      - os: ubuntu-latest
        target: x86_64-unknown-linux-musl
        artifact_name: cssh-linux-x86_64-musl
      - os: windows-latest
        target: x86_64-pc-windows-msvc
        artifact_name: cssh-windows-x86_64
  ```
- ステップ:
  1. ターゲット追加（`rustup target add`）
  2. musl-toolsのインストール（Linux muslの場合）
  3. `cargo build --release --workspace --target <target>`
  4. 3つのバイナリ収集:
     - `cssh` / `cssh.exe`
     - `cssh-agent` / `cssh-agent.exe`
     - `cexec` / `cexec.exe`
  5. アーカイブ作成:
     - Linux: `tar czf <artifact_name>.tar.gz <binaries>`
     - Windows: `7z a <artifact_name>.zip <binaries>`
  6. SHA256チェックサム生成
  7. GitHub Releaseへアップロード

---

### 3. Publish to crates.io Workflow (`.github/workflows/publish-crates.yml`)

**トリガー**:
- `workflow_dispatch`（手動実行）
- タグプッシュ後に`release.yml`から呼び出し（`workflow_call`）

**ジョブ構成**:

#### `publish`
- 依存順に4つのクレートを公開:
  1. `cssh-common`
  2. `cssh-agent`（cssh-commonに依存）
  3. `cssh-remote`（cssh-commonに依存）
  4. `cssh-cli`（cssh-commonに依存）
- 各公開の間に30秒待機（crates.ioのインデックス更新のため）
- 環境変数: `CARGO_REGISTRY_TOKEN`（GitHub Secretsから取得）
- ステップ:
  ```bash
  cargo publish -p cssh-common
  sleep 30
  cargo publish -p cssh-agent
  sleep 30
  cargo publish -p cssh-remote
  sleep 30
  cargo publish -p cssh-cli
  ```

---

### 4. Additional Configuration Files

#### `.github/dependabot.yml`
```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    groups:
      production-dependencies:
        dependency-type: "production"
      development-dependencies:
        dependency-type: "development"

  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "monthly"
```

#### `.github/workflows/security-audit.yml`
- **トリガー**:
  - スケジュール: 毎週月曜日
  - `pull_request`
  - `push` to main
- **ジョブ**:
  - `cargo audit`でセキュリティ脆弱性スキャン
  - `cargo deny check`でライセンスとセキュリティポリシー検証

---

### 5. Preparatory Tasks (実装前に必要)

#### 5.1 LICENSEファイルの追加
- `/LICENSE`にMITライセンステキストを追加
- テンプレート: https://opensource.org/licenses/MIT

#### 5.2 Cargo.tomlメタデータの追加
各クレートの`Cargo.toml`に以下を追加:
```toml
license = "MIT"
repository = "https://github.com/taiga533/cssh"
homepage = "https://github.com/taiga533/cssh"
description = "<クレートの説明>"
keywords = ["ssh", "remote", "command", "execution"]
categories = ["command-line-utilities", "network-programming"]
readme = "README.md"  # cssh-cliのみ
```

**各クレートの説明**:
- `cssh-common`: "Common protocol and types for cssh"
- `cssh-agent`: "Host-side agent for cssh command execution"
- `cssh-remote`: "Remote-side executable for cssh"
- `cssh-cli`: "SSH wrapper with host-side command execution via reverse port forwarding"

#### 5.3 コード品質問題の修正
- `cargo fmt --all`で既存のフォーマット問題を修正
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`で警告を全て解消

#### 5.4 MSRVの明示
ワークスペースの`Cargo.toml`に追加:
```toml
[workspace.package]
rust-version = "1.89.0"
```

各クレートの`Cargo.toml`に追加:
```toml
rust-version.workspace = true
```

---

## Critical Files

1. **/.github/workflows/ci.yml** - CI（テスト・lint）ワークフロー
2. **/.github/workflows/release.yml** - リリース自動化ワークフロー
3. **/.github/workflows/publish-crates.yml** - crates.io公開ワークフロー
4. **/.github/workflows/security-audit.yml** - セキュリティ監査ワークフロー
5. **/.github/dependabot.yml** - Dependabot設定
6. **/LICENSE** - MITライセンスファイル
7. **/Cargo.toml** - ワークスペース設定（MSRV追加）
8. **/cssh-common/Cargo.toml** - メタデータ追加
9. **/cssh-agent/Cargo.toml** - メタデータ追加
10. **/cssh-remote/Cargo.toml** - メタデータ追加
11. **/cssh-cli/Cargo.toml** - メタデータ追加

---

## Implementation Order

### フェーズ1: 準備作業
1. LICENSEファイルの追加
2. 全クレートのCargo.tomlにメタデータ追加
3. `cargo fmt --all`と`cargo clippy`の問題修正
4. MSRVの明示

### フェーズ2: CI基盤
1. `.github/workflows/ci.yml`の作成
2. format-check、lint、testジョブの実装
3. PRでの動作確認

### フェーズ3: E2Eテスト統合
1. `ci.yml`にe2e-testジョブを追加
2. Docker環境での安定性確認

### フェーズ4: リリース自動化
1. `.github/workflows/release.yml`の作成
2. クロスプラットフォームビルドの実装
3. テストリリース実行（v0.1.1など）

### フェーズ5: crates.io公開
1. `.github/workflows/publish-crates.yml`の作成
2. GitHub Secrets に `CARGO_REGISTRY_TOKEN` を設定
3. テスト公開（dry-run）

### フェーズ6: セキュリティとメンテナンス
1. `.github/dependabot.yml`の作成
2. `.github/workflows/security-audit.yml`の作成

---

## Verification Plan

### CI検証
1. 新しいブランチを作成してPRを開く
2. format-check、lint、test、e2e-testジョブが全て成功することを確認
3. 意図的にフォーマットエラーを入れてformat-checkが失敗することを確認
4. 意図的にテストエラーを入れてtestジョブが失敗することを確認

### リリース検証
1. テストタグ（例: `v0.1.1-test`）を作成してプッシュ
2. Release ワークフローが起動し、以下が成功することを確認:
   - GitHub Releaseの作成
   - Linux x64（gnu、musl）バイナリのビルドとアップロード
   - Windows x64バイナリのビルドとアップロード
   - 各アーカイブに3つのバイナリが含まれることを確認
   - SHA256チェックサムファイルが生成されることを確認
3. ダウンロードしたバイナリを実行して動作確認

### crates.io公開検証
1. `cargo publish --dry-run -p cssh-common`で事前確認
2. 手動トリガーで実際の公開を実行
3. crates.io上でパッケージが表示されることを確認
4. `cargo install cssh-cli`でインストールできることを確認

### セキュリティ監査検証
1. Security Audit ワークフローが定期的に実行されることを確認
2. 既知の脆弱性がある依存関係を追加して検出されることを確認

---

## Key Implementation Notes

### キャッシュ戦略
- `Swatinem/rust-cache@v2`を全ビルドジョブで使用
- OS、Rustバージョン、Cargo.lockのハッシュでキャッシュキーを自動生成
- ビルド時間を大幅に短縮（初回以降）

### エラーハンドリング
- ビルド失敗時: 詳細なログを出力
- テスト失敗時: テスト出力を保存、E2Eテストの場合はDockerログも保存
- リリース失敗時: 部分的なアセットをクリーンアップ

### セキュリティ
- `CARGO_REGISTRY_TOKEN`はGitHub Secretsで管理
- crates.io公開は手動トリガーまたはリリース後の承認フロー

### パフォーマンス
- 独立したジョブは並列実行
- マトリックス戦略で複数環境を同時テスト
- キャッシュで重複ビルドを削減
- 目標: CI全体で10-15分以内、リリースで20-30分以内

---

## Future Enhancements (今回は実装しない)

- macOS対応（x64、ARM64）の追加
- Linux ARM64対応の追加
- コードカバレッジレポート（codecov/coveralls）
- 自動リリースノート生成の強化
- Docker Imageの配布（GitHub Container Registry）
