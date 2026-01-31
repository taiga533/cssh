# Unix ドメインソケットフォワーディングへの移行計画

## 概要

リモート側の通信をTCPポートからUnixドメインソケット (UDS) に変更する。
ホスト側エージェントはTCP維持（変更なし）。SSH `-R` でリモートにUnixソケットを作成し、ポートファイル (`~/.cssh/agent_port`) にソケットパスを書き込んでcexecに伝える。Windowsホストの場合はTCPフォールバック。

## 設計判断の根拠

- **VSCode Remote SSH方式を参考**: リモート側でUnixソケットを使い、ホスト側はTCP維持
- **システムSSH維持**: russh等の組み込みは、SSHオプション透過パススルーの利点を失うため不採用
- **ポートファイル流用**: SetEnvはsshd側のAcceptEnv設定が必要なため不採用。既存の `~/.cssh/agent_port` にソケットパスを書き込む
- **Windowsフォールバック**: Win32-OpenSSHがstreamlocal未サポートのため、Windowsホストでは従来のTCPポート方式を維持

## アーキテクチャ

```
[Unixホスト]                              [リモート (Linux)]
cssh-agent (TCP 127.0.0.1:{port})
    ↑
    └── SSH -R /tmp/cssh-agent-{port}.sock:127.0.0.1:{port}
                                          /tmp/cssh-agent-{port}.sock (sshdが作成)
                                              ↑
                                          cexec → ~/.cssh/agent_port 読取
                                                → UnixStream::connect(path)

[Windowsホスト]                           [リモート (Linux)]
cssh-agent (TCP 127.0.0.1:{port})
    ↑
    └── SSH -R {port}:127.0.0.1:{port}    (TCP フォールバック)
                                          cexec → ~/.cssh/agent_port 読取
                                                → TcpStream::connect(127.0.0.1:{port})
```

## セキュリティ改善

| 観点 | TCP (現状) | UDS (変更後) |
|------|-----------|-------------|
| ポート露出 | localhost上の全プロセスが接続可能 | ファイルパーミッションで制御 |
| アクセス制御 | トークン認証のみ | ファイルパーミッション + トークン認証 |
| 盗聴リスク | ローカルネットワークスタック経由 | カーネル内バッファコピーのみ |

## 変更対象ファイル

### 1. `cssh-cli/src/ssh.rs`

**`run_ssh()`** — `-R` 引数をプラットフォームで分岐:
```rust
#[cfg(target_family = "unix")]
let forward_arg = format!("/tmp/cssh-agent-{}.sock:127.0.0.1:{}", agent_port, agent_port);
#[cfg(target_family = "windows")]
let forward_arg = format!("{}:127.0.0.1:{}", agent_port, agent_port);

cmd.arg("-R").arg(&forward_arg);
```

**`write_remote_port_file()`** — 接続情報のフォーマットを変更:
```
# Unixホスト時:
socket|/tmp/cssh-agent-{port}.sock|{token}

# Windowsホスト時:
tcp|{port}|{token}
```

### 2. `cssh-remote/src/main.rs`

**`AgentInfo`** → **`ConnectionInfo`** に変更:
```rust
pub(crate) enum ConnectionInfo {
    UnixSocket { path: String, token: String },
    Tcp { port: u16, token: String },
}
```

**`parse_connection_info()`** — 3フォーマット対応:
- `socket|/path/to.sock|token` → `UnixSocket`
- `tcp|12345|token` → `Tcp`
- `12345\ntoken` → `Tcp`（後方互換）

### 3. `cssh-remote/src/client.rs`

**`execute()`** — `ConnectionInfo` で分岐:
```rust
pub async fn execute(conn: ConnectionInfo, args: Vec<String>) -> Result<ExecuteResponse> {
    match conn {
        ConnectionInfo::UnixSocket { path, token } => {
            let stream = tokio::net::UnixStream::connect(&path).await?;
            // write_message / read_message はAsyncRead/WriteExtなのでそのまま動作
        }
        ConnectionInfo::Tcp { port, token } => {
            let stream = TcpStream::connect(("127.0.0.1", port)).await?;
            // 現在の実装と同じ
        }
    }
}
```

### 4. `cssh-common/src/platform.rs`

```rust
pub fn supports_unix_socket_forwarding() -> bool {
    cfg!(target_family = "unix")
}
```

### 変更不要

- `cssh-agent/src/server.rs` — ホスト側TCPリスナー変更なし
- `cssh-common/src/protocol.rs` — AsyncRead/WriteExtベースのため変更なし

## テスト

### ユニットテスト
- `parse_connection_info()`: socket形式、tcp形式、旧形式の各パース
- `execute()`: UnixSocket用テスト (`#[cfg(unix)]`、実UnixListenerを使用)
- `supports_unix_socket_forwarding()`: プラットフォーム判定
- 後方互換: 旧形式ポートファイルの読み込み

### 検証手順
```bash
cargo build --workspace
cargo test --workspace
bash e2e/run_e2e.sh
```

### 手動検証
- Linux/macOSホスト → Linuxリモート: cexecがUnixソケット経由でコマンド実行できること
- `/tmp/cssh-agent-{port}.sock` が作成されること
- `~/.cssh/agent_port` に `socket|...` 形式で書き込まれること
- 旧バージョンのcexecが新フォーマットのポートファイルでエラーになること（期待動作の確認）
