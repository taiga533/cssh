use cssh_common::error::Result;
use cssh_common::protocol::{ExecuteRequest, ExecuteResponse};
use std::process::Command;

/// Execute a command based on the request and return the result.
///
/// On Windows, commands are executed via `cmd /C` so that `.cmd` and
/// `.bat` scripts (e.g. VS Code's `code.cmd`) can be run directly.
pub fn execute(request: &ExecuteRequest) -> Result<ExecuteResponse> {
    if request.args.is_empty() {
        return Ok(ExecuteResponse {
            exit_code: 1,
            stdout: Vec::new(),
            stderr: b"no command specified".to_vec(),
        });
    }

    let output = build_command(&request.args)
        .output()
        .map_err(|e| cssh_common::error::CsshError::Execution(e.to_string()))?;

    Ok(ExecuteResponse {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

/// Build a platform-appropriate Command from the given arguments.
///
/// On Windows, wraps all commands with `cmd /C` so that `.cmd` and `.bat`
/// files can be executed without explicit `cmd /C` prefix.
/// On Unix, executes the command directly.
fn build_command(args: &[String]) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").args(args);
        cmd
    } else {
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_command_executes_correctly() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "hello".to_string()],
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
            args: vec!["exit".to_string(), "1".to_string()],
            token: String::new(),
        };

        // Act
        let response = execute(&request).unwrap();

        // Assert
        assert_ne!(response.exit_code, 0);
    }

    #[test]
    fn build_command_creates_platform_appropriate_command() {
        // Arrange
        let args = vec!["echo".to_string(), "test".to_string()];

        // Act
        let cmd = build_command(&args);

        // Assert
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "windows") {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "echo");
        }
    }
}
