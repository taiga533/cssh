use cssh_common::error::Result;
use cssh_common::protocol::{ExecuteRequest, ExecuteResponse};
use std::process::Command;

/// Execute a command based on the request and return the result.
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
    fn echo_command_executes_correctly() {
        // Arrange
        #[cfg(target_family = "unix")]
        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "hello".to_string()],
            token: String::new(),
        };

        #[cfg(target_family = "windows")]
        let request = ExecuteRequest {
            args: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo".to_string(),
                "hello".to_string(),
            ],
            token: String::new(),
        };

        // Act
        let response = execute(&request).unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&response.stdout).trim(), "hello");
    }

    #[test]
    fn returns_error_for_nonexistent_command() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["nonexistent_command_xyz".to_string()],
            token: String::new(),
        };

        // Act
        let result = execute(&request);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_response_for_empty_args() {
        // Arrange
        let request = ExecuteRequest {
            args: vec![],
            token: String::new(),
        };

        // Act
        let response = execute(&request).unwrap();

        // Assert
        assert_eq!(response.exit_code, 1);
    }

    #[test]
    fn nonzero_exit_code_is_handled_correctly() {
        // Arrange
        #[cfg(target_family = "unix")]
        let request = ExecuteRequest {
            args: vec!["false".to_string()],
            token: String::new(),
        };

        #[cfg(target_family = "windows")]
        let request = ExecuteRequest {
            args: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "exit".to_string(),
                "1".to_string(),
            ],
            token: String::new(),
        };

        // Act
        let response = execute(&request).unwrap();

        // Assert
        assert_ne!(response.exit_code, 0);
    }
}
