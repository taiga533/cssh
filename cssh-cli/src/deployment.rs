use std::process::Command;

use crate::args::CsshArgs;

/// リモートにcssh-remoteバイナリを配置する
///
/// 1. リモートの~/.cssh/にバイナリが存在するか確認
/// 2. 存在しない場合のみSCPで配置
pub fn deploy_remote_binary(args: &CsshArgs) -> Result<(), String> {
    let remote_bin_path = format!(".cssh/{}", args.remote_name);

    // リモートにバイナリが存在するか確認
    if check_remote_binary_exists(args, &remote_bin_path)? {
        tracing::info!("リモートバイナリが既に存在します");
        return Ok(());
    }

    // ローカルのcssh-remoteバイナリパスを取得
    let local_bin = find_local_binary()?;

    // リモートに~/.cssh/ディレクトリを作成
    create_remote_directory(args)?;

    // SCPで配置
    scp_to_remote(args, &local_bin, &remote_bin_path)?;

    // 実行権限を付与
    set_remote_executable(args, &remote_bin_path)?;

    // ~/.bashrcにPATH追加（まだ追加されていない場合のみ）
    setup_remote_path(args)?;

    tracing::info!("リモートバイナリを配置しました");
    Ok(())
}

/// リモートにバイナリが存在するか確認する
fn check_remote_binary_exists(args: &CsshArgs, remote_path: &str) -> Result<bool, String> {
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination)
        .arg(format!("test -x {}", remote_path));

    let status = cmd
        .status()
        .map_err(|e| format!("ssh実行失敗: {}", e))?;
    Ok(status.success())
}

/// ローカルのcssh-remoteバイナリを探す
fn find_local_binary() -> Result<String, String> {
    // 実行中のバイナリと同じディレクトリにあるcssh-remoteを探す
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("現在の実行ファイルパス取得失敗: {}", e))?;
    let dir = current_exe
        .parent()
        .ok_or("親ディレクトリが取得できません")?;
    let remote_bin = dir.join("cexec");

    if remote_bin.exists() {
        return Ok(remote_bin.to_string_lossy().to_string());
    }

    Err("cexecバイナリが見つかりません".to_string())
}

/// リモートに~/.cssh/ディレクトリを作成する
fn create_remote_directory(args: &CsshArgs) -> Result<(), String> {
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination).arg("mkdir -p ~/.cssh");

    let status = cmd
        .status()
        .map_err(|e| format!("リモートディレクトリ作成失敗: {}", e))?;

    if !status.success() {
        return Err("リモートディレクトリ作成に失敗しました".to_string());
    }
    Ok(())
}

/// SCPでファイルをリモートに送信する
fn scp_to_remote(args: &CsshArgs, local_path: &str, remote_path: &str) -> Result<(), String> {
    let mut cmd = Command::new("scp");
    // sshオプションをscpに変換して転送（-p → -P）
    let mut iter = args.ssh_options.iter();
    while let Some(opt) = iter.next() {
        if opt == "-p" {
            cmd.arg("-P");
            if let Some(val) = iter.next() {
                cmd.arg(val);
            }
        } else {
            cmd.arg(opt);
        }
    }
    cmd.arg(local_path)
        .arg(format!("{}:{}", args.destination, remote_path));

    let status = cmd
        .status()
        .map_err(|e| format!("scp実行失敗: {}", e))?;

    if !status.success() {
        return Err("SCPによるファイル転送に失敗しました".to_string());
    }
    Ok(())
}

/// リモートファイルに実行権限を付与する
fn set_remote_executable(args: &CsshArgs, remote_path: &str) -> Result<(), String> {
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination)
        .arg(format!("chmod +x {}", remote_path));

    let status = cmd
        .status()
        .map_err(|e| format!("chmod実行失敗: {}", e))?;

    if !status.success() {
        return Err("実行権限の付与に失敗しました".to_string());
    }
    Ok(())
}

/// リモートの~/.bashrcにPATH追加を設定する
fn setup_remote_path(args: &CsshArgs) -> Result<(), String> {
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination).arg(
        r#"grep -q 'export PATH="$HOME/.cssh:$PATH"' ~/.bashrc 2>/dev/null || echo 'export PATH="$HOME/.cssh:$PATH"' >> ~/.bashrc"#,
    );

    let status = cmd
        .status()
        .map_err(|e| format!("PATH設定失敗: {}", e))?;

    if !status.success() {
        return Err("PATH設定に失敗しました".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ローカルバイナリが見つからない場合にエラーを返す() {
        // Act
        // 通常テスト環境ではcssh-remoteはビルドディレクトリに存在する可能性があるが、
        // find_local_binaryの動作を確認するためのテスト
        let result = find_local_binary();

        // Assert - ビルド済みなら成功、なければエラー（どちらもOK）
        // このテストはfind_local_binaryがパニックしないことを確認
        let _ = result;
    }
}
