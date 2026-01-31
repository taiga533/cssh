# cssh

SSH をラップし、接続時にホスト側エージェントを自動起動する CLI ツール。逆ポートフォワーディングにより、リモートからホスト側のコマンドを実行できる。

## 概要

```
┌──────────────┐        SSH (-R)         ┌──────────────┐
│  ホスト側    │◄────────────────────────│ リモート側   │
│              │                         │              │
│  cssh-agent  │◄───── TCP (JSON) ───────│  cexec       │
│(コマンド実行)│                         │(コマンド送信)│
└──────────────┘                         └──────────────┘
```

`cssh` は以下を自動で行う:

1. ホスト側で `cssh-agent` をバックグラウンド起動
2. `-R` 逆ポートフォワーディング付きで SSH 接続
3. リモートで `cexec <コマンド>` を実行すると、ホスト側でそのコマンドが実行される

## インストール

### ホスト側（`cssh` を実行するマシン）

`cssh` と `cssh-agent` をインストールする:

```bash
cargo install --git https://github.com/taiga533/cssh cssh-cli cssh-agent
```

`cssh-agent` は `cssh` が自動起動するため、PATH に含まれている必要がある。

### リモート側（`cexec` を実行するマシン）

各リモートホストに `cexec` をインストールする:

```bash
cargo install --git https://github.com/taiga533/cssh cssh-remote
```

`cexec` がリモートホストの PATH に含まれている必要がある。

## 使い方

### 基本

```bash
cssh user@example.com
```

### SSH オプション付き

SSH オプション（`-p`, `-i` 等）はそのまま ssh に転送される。

```bash
cssh -p 2222 -i ~/.ssh/id_rsa user@example.com
```

### リモートコマンド指定

`--` の後にリモートで実行するコマンドを指定できる。

```bash
cssh user@example.com -- ls -la
```

### リモートからホスト側コマンドを実行

SSH 接続後、リモートのシェルで:

```bash
cexec echo "hello from host"
cexec cat /etc/hostname
cexec pwd
```

### cssh 独自オプション

| オプション | 説明 | デフォルト |
|---|---|---|
| `--listen-port <PORT>` | エージェントポート | 自動割り当て |

```bash
cssh --listen-port 8080 user@example.com
```

## SSH サーバー設定

`CSSH_PORT` 環境変数をリモートに送信するため、SSH サーバーの `sshd_config` に以下を追加する:

```
AcceptEnv CSSH_PORT
```

## 通信プロトコル

JSON over TCP（長さプレフィクス付き）。

```
[4バイト: メッセージ長（ビッグエンディアン）][JSON ペイロード]
```

**リクエスト:**

```json
{ "args": ["echo", "hello"] }
```

**レスポンス:**

```json
{ "exit_code": 0, "stdout": [...], "stderr": [...] }
```

## プロジェクト構成

```
cssh/
├── Cargo.toml          # Workspace 定義
├── cssh-common/        # 共通ライブラリ（プロトコル定義、エラー型）
├── cssh-agent/         # ホスト側エージェント（TCP サーバー、コマンド実行）
├── cssh-remote/        # リモート実行ファイル（TCP クライアント）
├── cssh-cli/           # cssh コマンド本体（CLI 引数解析、SSH 接続、配置）
└── e2e/                # E2E テスト（Docker SSH 環境）
```

## テスト

```bash
# 全テスト実行
cargo test --workspace

# E2E テスト（Docker 必須）
bash e2e/run_e2e.sh
```

## ライセンス

MIT
