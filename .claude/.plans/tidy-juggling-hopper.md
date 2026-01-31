# セキュリティ強化計画

## 1. 平文トークンファイルのTOCTOU対策 (cssh-cli/src/ssh.rs:137-140)

**現状**: `mkdir -p && printf > file && chmod 600 file` — ファイル作成とchmod間にTOCTOU競合あり

**対策**: umask 077 をサブシェルで適用し、作成時点からパーミッション600にする

変更前:
```
mkdir -p ~/.cssh && printf '%s\n' '...' > ~/.cssh/agent_port && chmod 600 ~/.cssh/agent_port
```

変更後:
```
mkdir -p ~/.cssh && (umask 077 && printf '%s\n' '...' > ~/.cssh/agent_port)
```

**修正ファイル**: `cssh-cli/src/ssh.rs` L137-140

---

## 2. エラーメッセージの情報漏洩 (cssh-agent/src/server.rs:134-138)

**現状**: `executor::execute` のエラーが `format!("execution error: {}", e)` でそのままリモートに返される。

**対策**: リモートには `"command execution failed"` のみ返し、詳細は `error!()` でローカルログに記録。

変更前:
```rust
let response = executor::execute(&request).unwrap_or_else(|e| ExecuteResponse {
    exit_code: 1,
    stdout: Vec::new(),
    stderr: format!("execution error: {}", e).into_bytes(),
});
```

変更後:
```rust
let response = executor::execute(&request).unwrap_or_else(|e| {
    error!("command execution failed: {}", e);
    ExecuteResponse {
        exit_code: 1,
        stdout: Vec::new(),
        stderr: b"command execution failed".to_vec(),
    }
});
```

**修正ファイル**: `cssh-agent/src/server.rs` L134-138

---

## 修正箇所まとめ

| # | ファイル | 変更内容 |
|---|---------|---------|
| 1 | `cssh-cli/src/ssh.rs` L137-140 | umask 077 でファイル作成（TOCTOU解消） |
| 2 | `cssh-agent/src/server.rs` L134-138 | エラー詳細をローカルログのみに制限 |
| 3 | 各変更に対するテスト追加 | |

## 検証方法

1. `cargo build --workspace` でビルド確認
2. `cargo test --workspace` で既存テスト通過確認
3. server.rsのテスト: 実行エラー時にリモート返却メッセージにシステムパス等が含まれないことを確認
4. ssh.rsのテスト: 生成されるコマンド文字列にumaskが含まれることを確認
