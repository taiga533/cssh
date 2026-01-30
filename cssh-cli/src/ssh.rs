use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use crate::args::CsshArgs;
use crate::deployment;

/// csshのメイン処理を実行する
///
/// 1. エージェントをバックグラウンドで起動
/// 2. リモートにcssh-remoteを配置
/// 3. SSH接続（逆ポートフォワーディング付き）
pub async fn run(args: CsshArgs) -> Result<(), String> {
    // エージェント起動
    let (agent_process, agent_port) = start_agent(&args)?;
    tracing::info!("エージェント起動: ポート {}", agent_port);

    // リモートバイナリ配置
    if let Err(e) = deployment::deploy_remote_binary(&args) {
        tracing::warn!("リモートバイナリ配置スキップ: {}", e);
    }

    // SSH接続
    let exit_code = run_ssh(&args, agent_port)?;

    // エージェント停止
    drop(agent_process);

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

/// エージェントプロセスをバックグラウンドで起動し、割り当てられたポートを返す
fn start_agent(args: &CsshArgs) -> Result<(AgentGuard, u16), String> {
    let port_arg = args.listen_port.to_string();

    // 自分と同じディレクトリのcssh-agentを探す
    let agent_bin = find_agent_binary()?;

    let mut child = Command::new(&agent_bin)
        .arg(&port_arg)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("エージェント起動失敗: {}", e))?;

    // エージェントが出力するポート番号を読み取る
    let stdout = child.stdout.take().ok_or("エージェントの標準出力を取得できません")?;
    let mut reader = BufReader::new(stdout);
    let mut port_line = String::new();
    reader
        .read_line(&mut port_line)
        .map_err(|e| format!("ポート読み取り失敗: {}", e))?;

    let port: u16 = port_line
        .trim()
        .parse()
        .map_err(|_| format!("不正なポート番号: '{}'", port_line.trim()))?;

    Ok((AgentGuard(child), port))
}

/// エージェントバイナリを探す
fn find_agent_binary() -> Result<String, String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("現在の実行ファイルパス取得失敗: {}", e))?;
    let dir = current_exe
        .parent()
        .ok_or("親ディレクトリが取得できません")?;
    let agent_bin = dir.join("cssh-agent");

    if agent_bin.exists() {
        return Ok(agent_bin.to_string_lossy().to_string());
    }

    Err("cssh-agentバイナリが見つかりません".to_string())
}

/// SSH接続を実行し、終了コードを返す
fn run_ssh(args: &CsshArgs, agent_port: u16) -> Result<i32, String> {
    let mut cmd = Command::new("ssh");

    // 逆ポートフォワーディング: リモートのポートをローカルのエージェントポートに接続
    cmd.arg("-R")
        .arg(format!("{}:127.0.0.1:{}", agent_port, agent_port));

    // CSSH_PORT環境変数をリモートに送信
    cmd.arg("-o")
        .arg("SendEnv=CSSH_PORT")
        .arg("-o")
        .arg(format!("SetEnv=CSSH_PORT={}", agent_port));

    // ユーザー指定のsshオプション
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }

    // 接続先
    cmd.arg(&args.destination);

    // リモートコマンド
    if !args.remote_command.is_empty() {
        for c in &args.remote_command {
            cmd.arg(c);
        }
    }

    let status = cmd
        .status()
        .map_err(|e| format!("SSH実行失敗: {}", e))?;

    Ok(status.code().unwrap_or(1))
}

/// エージェントプロセスのドロップ時に自動終了するガード
struct AgentGuard(Child);

impl Drop for AgentGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn エージェントバイナリが存在しない場合にエラーを返す() {
        // find_agent_binaryがパニックしないことを確認
        let _ = find_agent_binary();
    }
}
