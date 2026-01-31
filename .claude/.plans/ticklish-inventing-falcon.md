# セキュリティ対策実装計画

## 1. Windowsコマンドインジェクション対策

**対象ファイル:** `cssh-agent/src/executor.rs`

**現状:** `build_command` (L34-44) で Windows では全コマンドを `cmd /C` 経由で実行。`&`, `|`, `;` 等のメタキャラクタが解釈される。

**対策:**
- コマンド名の拡張子が `.cmd` / `.bat` の場合のみ `cmd /C` を使用
- それ以外は Unix 同様に直接実行 (`Command::new(&args[0])`)
- `.cmd`/`.bat` 経由の場合、引数を `^` でエスケープする関数 `escape_cmd_arg()` を追加
  - エスケープ対象: `&`, `|`, `(`, `)`, `<`, `>`, `^`, `"`, `%`, `!`

**テスト:**
- `build_command` が `.bat` でのみ `cmd /C` を使うことを検証
- `escape_cmd_arg` のエスケープ処理を検証
- 直接実行パスの動作確認

## 2. ブルートフォース対策

**対象ファイル:** `cssh-agent/src/server.rs`

**現状:** `handle_connection` (L33-77) で認証失敗時にエラーを返して即切断するが、新規接続に対するレート制限がない。

**対策:**
- `Arc<Mutex<BruteForceGuard>>` を `run()` で生成し、各接続ハンドラに渡す
- `BruteForceGuard` の仕様:
  - 連続認証失敗回数をカウント
  - **3回**連続失敗で **10秒**のディレイを `accept` 後に挿入
  - 認証成功時にカウンタリセット
- トークン比較を constant-time に変更 (`subtle` crateの `ConstantTimeEq` を使用)

**テスト:**
- 連続失敗時にディレイが発生することを検証（時間計測）
- 成功後にカウンタがリセットされることを検証
- constant-time 比較が正しく動作することを検証

## 3. SSH引数の検証 → 対応しない

SSHコマンドのラッパーという性質上、SSHオプションの制限は行わない。

## 依存関係の追加

- `subtle` crate (workspace.dependencies に追加) — constant-time 比較用

## 検証方法

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```
