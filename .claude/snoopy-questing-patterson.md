# cssh 実装計画

## 概要
sshをラップするRust CLIツール「cssh」を実装する。接続時にホスト側エージェントを起動し、逆ポートフォワーディング経由でリモートからホスト側コマンドを実行可能にする。

## プロジェクト構成（Cargo Workspace）

```
cssh/
├── Cargo.toml          # workspace root
├── cssh-common/        # 共通ライブラリ（プロトコル定義、エラー型）
├── cssh-agent/         # ホスト側バックグラウンドエージェント
├── cssh-remote/        # リモートに配置する実行可能ファイルA
└── cssh-cli/           # csshコマンド本体
```

## 通信プロトコル: JSON over TCP（長さプレフィクス付き）

```
[4bytes: メッセージ長][JSONペイロード]
```

- **リクエスト**: `{ args: Vec<String> }`
- **レスポンス**: `{ exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8> }`

## CLI設計

```bash
cssh [SSH_OPTIONS...] <USER@HOST> -- <COMMAND>

# sshオプションはそのままsshに渡される
# 例: cssh -p 2222 -i ~/.ssh/id_rsa user@example.com -- cat
# → リモートで "cexec file.txt" 実行 → ホストで "cat file.txt" 実行

cssh独自オプション（--の前に指定）:
  --remote-name <NAME>   リモート実行可能ファイル名 [デフォルト: cexec]
  --listen-port <PORT>   エージェントポート [デフォルト: 自動]
```

sshに渡す引数（-p, -i等）はそのままsshコマンドに転送される。

## リモートバイナリ配置戦略
- 接続時にリモートの`~/.cssh/`にcssh-remoteバイナリが既に存在するか確認
- 存在しない、またはバージョンが古い場合のみSCPで配置
- リモートのアーキテクチャは`uname -m`で検出
- 初期実装ではホストと同一アーキテクチャのみ対応

## 実装順序

### Phase 1: 基盤
1. **Cargo Workspaceセットアップ** — ルートCargo.toml、各クレート作成
2. **cssh-common実装** — プロトコル定義（`ExecuteRequest`/`ExecuteResponse`）、エラー型、シリアライズユーティリティ + テスト

### Phase 2: コア機能
3. **cssh-agent実装** — TCPサーバー（tokio）、コマンド実行（`std::process::Command`）、ロックファイル管理 + テスト
4. **cssh-remote実装** — TCPクライアント、引数送信、結果表示 + テスト
5. **ローカル結合テスト** — エージェント↔リモート間の通信テスト

### Phase 3: SSH統合
6. **cssh-cli実装** — CLI引数解析、エージェント起動管理、SSH接続（sshオプションをそのまま転送、`-R`逆ポートフォワーディング追加）、SCPによるリモートファイル配置（インストール要否判断付き） + テスト

### Phase 4: 仕上げ
7. **E2Eテスト** — Docker SSH環境でのテスト
8. **ドキュメント** — README.md

## 主要依存クレート
- `clap = "4.5"` — CLI引数解析
- `tokio = "1.41"` — 非同期ランタイム
- `serde = "1.0"` / `serde_json = "1.0"` — シリアライズ
- `thiserror = "2.0"` — エラー型定義
- `tracing = "0.1"` — ログ
- `daemonize = "0.5"` — エージェントのデーモン化

## 主要ファイル
- `Cargo.toml` — Workspace定義
- `cssh-common/src/protocol.rs` — プロトコル定義（全クレートの基盤）
- `cssh-agent/src/server.rs` — TCPサーバー
- `cssh-agent/src/executor.rs` — コマンド実行
- `cssh-remote/src/main.rs` — リモート実行可能ファイル
- `cssh-cli/src/main.rs` — csshエントリーポイント
- `cssh-cli/src/ssh.rs` — SSH接続・逆ポートフォワーディング
- `cssh-cli/src/deployment.rs` — リモートバイナリ配置（インストール要否判断）

## 検証方法
1. `cargo test --workspace` で全テスト実行
2. ローカルでエージェント起動 → cssh-remoteから接続してコマンド実行確認
3. Docker SSH環境でE2Eテスト
