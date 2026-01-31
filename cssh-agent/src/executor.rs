use cssh_common::error::Result;
use cssh_common::protocol::{ExecuteRequest, ExecuteResponse};
use std::process::Command;

/// Execute a command based on the request and return the result.
///
/// On Windows, `.cmd` and `.bat` files are executed via `cmd /C` with
/// arguments escaped to prevent command injection. All other commands
/// are executed directly.
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

/// Check whether a command name has a `.cmd` or `.bat` extension.
fn is_cmd_or_bat(program: &str) -> bool {
    let lower = program.to_ascii_lowercase();
    lower.ends_with(".cmd") || lower.ends_with(".bat")
}

/// Escape a single argument for use with `cmd /C` on Windows.
///
/// Prefixes each shell metacharacter with `^` to prevent interpretation
/// by `cmd.exe`.
fn escape_cmd_arg(arg: &str) -> String {
    let mut escaped = String::with_capacity(arg.len());
    for ch in arg.chars() {
        if matches!(
            ch,
            '&' | '|' | '(' | ')' | '<' | '>' | '^' | '"' | '%' | '!'
        ) {
            escaped.push('^');
        }
        escaped.push(ch);
    }
    escaped
}

/// Build a platform-appropriate Command from the given arguments.
///
/// On Windows, wraps commands with `cmd /C` only when the program has a
/// `.cmd` or `.bat` extension, applying argument escaping to prevent
/// command injection. All other commands are executed directly.
/// On Unix, executes the command directly.
fn build_command(args: &[String]) -> Command {
    if cfg!(target_os = "windows") && is_cmd_or_bat(&args[0]) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(&args[0]);
        for arg in &args[1..] {
            cmd.arg(escape_cmd_arg(arg));
        }
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
            args: vec![
                "cmd".to_string(),
                "/c".to_string(),
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

    #[test]
    fn build_command_creates_platform_appropriate_command() {
        // Arrange
        let args = vec!["echo".to_string(), "test".to_string()];

        // Act
        let cmd = build_command(&args);

        // Assert
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "windows") {
            // "echo" is not .cmd/.bat, so it should be executed directly
            assert_eq!(program, "echo");
        } else {
            assert_eq!(program, "echo");
        }
    }

    #[test]
    fn build_command_uses_cmd_for_bat_files() {
        // Arrange
        let args = vec!["script.bat".to_string(), "arg1".to_string()];

        // Act
        let cmd = build_command(&args);

        // Assert
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "windows") {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "script.bat");
        }
    }

    #[test]
    fn build_command_uses_cmd_for_cmd_files() {
        // Arrange
        let args = vec!["code.CMD".to_string(), "file.txt".to_string()];

        // Act
        let cmd = build_command(&args);

        // Assert
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "windows") {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "code.CMD");
        }
    }

    #[test]
    fn build_command_does_not_use_cmd_for_regular_executables() {
        // Arrange
        let args = vec!["git".to_string(), "status".to_string()];

        // Act
        let cmd = build_command(&args);

        // Assert
        let program = cmd.get_program().to_string_lossy().to_string();
        assert_eq!(program, "git");
    }

    #[test]
    fn escape_cmd_arg_escapes_metacharacters() {
        // Arrange & Act & Assert
        assert_eq!(escape_cmd_arg("hello"), "hello");
        assert_eq!(escape_cmd_arg("a&b"), "a^&b");
        assert_eq!(escape_cmd_arg("a|b"), "a^|b");
        assert_eq!(escape_cmd_arg("a(b)"), "a^(b^)");
        assert_eq!(escape_cmd_arg("a<b>c"), "a^<b^>c");
        assert_eq!(escape_cmd_arg("a^b"), "a^^b");
        assert_eq!(escape_cmd_arg("a\"b"), "a^\"b");
        assert_eq!(escape_cmd_arg("a%b"), "a^%b");
        assert_eq!(escape_cmd_arg("a!b"), "a^!b");
    }

    #[test]
    fn escape_cmd_arg_handles_multiple_metacharacters() {
        // Arrange
        let input = "echo hello & del /q * | rm -rf";

        // Act
        let result = escape_cmd_arg(input);

        // Assert
        assert_eq!(result, "echo hello ^& del /q * ^| rm -rf");
    }

    #[test]
    fn is_cmd_or_bat_detects_correctly() {
        // Arrange & Act & Assert
        assert!(is_cmd_or_bat("script.bat"));
        assert!(is_cmd_or_bat("script.BAT"));
        assert!(is_cmd_or_bat("code.cmd"));
        assert!(is_cmd_or_bat("code.CMD"));
        assert!(!is_cmd_or_bat("git"));
        assert!(!is_cmd_or_bat("python.exe"));
        assert!(!is_cmd_or_bat("script.sh"));
    }
}
