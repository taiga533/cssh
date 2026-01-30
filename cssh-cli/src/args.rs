/// cssh CLI引数の解析結果
#[derive(Debug, Clone)]
pub struct CsshArgs {
    /// リモート実行可能ファイル名
    pub remote_name: String,
    /// エージェントのリッスンポート（0=自動）
    pub listen_port: u16,
    /// sshに渡すオプション引数（-p, -i等）
    pub ssh_options: Vec<String>,
    /// 接続先 (user@host)
    pub destination: String,
    /// リモートで実行するコマンド（--の後）
    pub remote_command: Vec<String>,
}

/// CLI引数を解析する
///
/// 書式: cssh [--remote-name NAME] [--listen-port PORT] [SSH_OPTIONS...] <destination> [-- <COMMAND...>]
pub fn parse() -> Result<CsshArgs, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    parse_from(&args)
}

/// 指定された引数リストから解析する（テスト用）
pub fn parse_from(args: &[String]) -> Result<CsshArgs, String> {
    let mut remote_name = "cexec".to_string();
    let mut listen_port: u16 = 0;
    let mut ssh_options = Vec::new();
    let mut destination = None;
    let mut remote_command = Vec::new();

    let mut i = 0;
    let mut found_separator = false;

    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            found_separator = true;
            i += 1;
            break;
        }

        if arg == "--remote-name" {
            i += 1;
            remote_name = args
                .get(i)
                .ok_or("--remote-name に値が必要です")?
                .clone();
        } else if arg == "--listen-port" {
            i += 1;
            listen_port = args
                .get(i)
                .ok_or("--listen-port に値が必要です")?
                .parse()
                .map_err(|_| "--listen-port は数値で指定してください")?;
        } else if arg.starts_with('-') {
            // sshオプション
            ssh_options.push(arg.clone());
            // -p, -i, -l, -o 等は次の引数も値として取る
            if matches!(arg.as_str(), "-p" | "-i" | "-l" | "-o" | "-F" | "-J" | "-W") {
                i += 1;
                ssh_options.push(
                    args.get(i)
                        .ok_or(format!("{} に値が必要です", arg))?
                        .clone(),
                );
            }
        } else {
            // 接続先
            destination = Some(arg.clone());
        }

        i += 1;
    }

    if found_separator {
        remote_command = args[i..].to_vec();
    }

    let destination = destination.ok_or("接続先を指定してください")?;

    Ok(CsshArgs {
        remote_name,
        listen_port,
        ssh_options,
        destination,
        remote_command,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 基本的な引数を正しく解析する() {
        // Arrange
        let args = s(&["user@host"]);

        // Act
        let result = parse_from(&args).unwrap();

        // Assert
        assert_eq!(result.destination, "user@host");
        assert_eq!(result.remote_name, "cexec");
        assert_eq!(result.listen_port, 0);
        assert!(result.ssh_options.is_empty());
        assert!(result.remote_command.is_empty());
    }

    #[test]
    fn sshオプション付きの引数を解析する() {
        // Arrange
        let args = s(&["-p", "2222", "-i", "~/.ssh/id_rsa", "user@host"]);

        // Act
        let result = parse_from(&args).unwrap();

        // Assert
        assert_eq!(result.destination, "user@host");
        assert_eq!(result.ssh_options, s(&["-p", "2222", "-i", "~/.ssh/id_rsa"]));
    }

    #[test]
    fn cssh独自オプションを解析する() {
        // Arrange
        let args = s(&["--remote-name", "myexec", "--listen-port", "8080", "user@host"]);

        // Act
        let result = parse_from(&args).unwrap();

        // Assert
        assert_eq!(result.remote_name, "myexec");
        assert_eq!(result.listen_port, 8080);
    }

    #[test]
    fn セパレータ後のリモートコマンドを解析する() {
        // Arrange
        let args = s(&["user@host", "--", "cat", "file.txt"]);

        // Act
        let result = parse_from(&args).unwrap();

        // Assert
        assert_eq!(result.remote_command, s(&["cat", "file.txt"]));
    }

    #[test]
    fn 接続先がない場合にエラーを返す() {
        // Arrange
        let args = s(&["-p", "2222"]);

        // Act
        let result = parse_from(&args);

        // Assert
        assert!(result.is_err());
    }
}
