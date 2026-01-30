use cssh_common::error::Result;
use cssh_common::protocol::{ExecuteRequest, ExecuteResponse};
use std::process::Command;

/// リクエストに基づいてコマンドを実行し、結果を返す
pub fn execute(request: &ExecuteRequest) -> Result<ExecuteResponse> {
    if request.args.is_empty() {
        return Ok(ExecuteResponse {
            exit_code: 1,
            stdout: Vec::new(),
            stderr: b"no command specified".to_vec(),
        });
    }

    let output = Command::new(&request.args[0])
        .args(&request.args[1..])
        .output()
        .map_err(|e| cssh_common::error::CsshError::Execution(e.to_string()))?;

    Ok(ExecuteResponse {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echoコマンドが正しく実行される() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "hello".to_string()],
        };

        // Act
        let response = execute(&request).unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&response.stdout).trim(), "hello");
    }

    #[test]
    fn 存在しないコマンドでエラーを返す() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["nonexistent_command_xyz".to_string()],
        };

        // Act
        let result = execute(&request);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn 空のargsでエラーレスポンスを返す() {
        // Arrange
        let request = ExecuteRequest { args: vec![] };

        // Act
        let response = execute(&request).unwrap();

        // Assert
        assert_eq!(response.exit_code, 1);
    }

    #[test]
    fn 終了コード非ゼロのコマンドが正しく処理される() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["false".to_string()],
        };

        // Act
        let response = execute(&request).unwrap();

        // Assert
        assert_ne!(response.exit_code, 0);
    }
}
