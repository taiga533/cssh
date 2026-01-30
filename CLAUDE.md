# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ビルド・テストコマンド

```bash
# ワークスペース全体のビルド
cargo build --workspace

# リリースビルド
cargo build --release --workspace

# 全テスト実行
cargo test --workspace

# 特定クレートのテスト
cargo test -p cssh-common
cargo test -p cssh-agent
cargo test -p cssh-remote
cargo test -p cssh-cli

# 特定のテスト関数を実行
cargo test -p cssh-common -- プロトコル名の一部

# E2Eテスト（Docker必須）
bash e2e/run_e2e.sh
```

ビルド成果物は3つのバイナリ: `cssh`（CLI本体）、`cssh-agent`（ホスト側エージェント）、`cexec`（リモート実行ファイル）。

## アーキテクチャ

SSHをラップし、逆ポートフォワーディング経由でリモートからホスト側コマンドを実行するツール。

### クレート構成と依存関係

```
cssh-cli  ──→ cssh-common ←── cssh-agent
                   ↑
               cssh-remote
```

- **cssh-common**: 全クレートの基盤。プロトコル定義（`ExecuteRequest`/`ExecuteResponse`）、エラー型（`CsshError`）、TCP上のJSON読み書き関数を提供
- **cssh-agent**: ホスト側で動作するTCPサーバー。リクエストを受けて`std::process::Command`でコマンド実行し結果を返す。ポート番号をstdoutに出力して親プロセス（cssh-cli）に通知する（tracingログはstderrに出力）
- **cssh-remote**: リモート側で動作するTCPクライアント（バイナリ名: `cexec`）。`CSSH_PORT`環境変数でエージェントのポートを取得し接続する
- **cssh-cli**: CLI本体（バイナリ名: `cssh`）。エージェント起動 → cexecのSCP配置 → SSH接続（`-R`逆ポートフォワーディング + `SetEnv`）を一括実行。引数解析は手動パース（`args.rs`）でSSHオプションをそのまま転送する

### 通信プロトコル

JSON over TCP、4バイトビッグエンディアン長さプレフィクス付き。最大メッセージサイズ16MB。`protocol.rs`の`read_message`/`write_message`が非同期I/O、`encode_message`/`decode_message`が同期版。

### 主要な設計判断

- エージェントのポート通知はstdout最初の1行で行う。親プロセスが`BufReader::read_line`で読み取る
- `AgentGuard`（`ssh.rs`）がRAIIパターンでエージェントプロセスをdrop時に自動kill
- SCPでリモートに配置する際、SSHの`-p`オプションをSCPの`-P`に変換する処理が`deployment.rs`にある
- 依存クレートのバージョンはワークスペースルートの`[workspace.dependencies]`で一元管理

## テスト規約

- Arrange - Act - Assert の順序で記述
- テストケース名は日本語で、何をテストしているか明確に（例: `echoコマンドが正しく実行される`）
- モックは最小限。TCPテストでは実際に`TcpListener`を`127.0.0.1:0`でバインドして使用
- 結合テストは`cssh-common/tests/integration_test.rs`に配置
