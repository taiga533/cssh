mod client;

/// Resolve the agent port by reading the port file.
///
/// Reads `~/.cssh/agent_port` which is written by cssh before
/// establishing the SSH connection.
fn resolve_port() -> Result<u16, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let port_file = format!("{}/.cssh/agent_port", home);
    let content = std::fs::read_to_string(&port_file)
        .map_err(|e| format!("failed to read {}: {}", port_file, e))?;
    content
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("invalid port in {}: '{}'", port_file, content.trim()))
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: cexec <command> [args...]");
        std::process::exit(1);
    }

    let port = match resolve_port() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cssh-remote error: {}", e);
            std::process::exit(1);
        }
    };

    match client::execute(port, args).await {
        Ok(response) => {
            use std::io::Write;
            std::io::stdout().write_all(&response.stdout).ok();
            std::io::stderr().write_all(&response.stderr).ok();
            std::process::exit(response.exit_code);
        }
        Err(e) => {
            eprintln!("cssh-remote error: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_port_reads_port_from_file() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let cssh_dir = fake_home.path().join(".cssh");
        std::fs::create_dir_all(&cssh_dir).unwrap();
        std::fs::write(cssh_dir.join("agent_port"), "54321\n").unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_port();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        assert_eq!(result.unwrap(), 54321);
    }

    #[test]
    fn resolve_port_returns_error_when_file_missing() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_port();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to read"));
    }

    #[test]
    fn resolve_port_returns_error_for_invalid_content() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let cssh_dir = fake_home.path().join(".cssh");
        std::fs::create_dir_all(&cssh_dir).unwrap();
        std::fs::write(cssh_dir.join("agent_port"), "not_a_number\n").unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_port();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid port"));
    }
}
