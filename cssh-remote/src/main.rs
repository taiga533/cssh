mod client;

/// Agent connection info (port and authentication token).
#[derive(Debug)]
struct AgentInfo {
    port: u16,
    token: String,
}

/// Resolve the agent port and token by reading the port file.
///
/// Reads `~/.cssh/agent_port` which contains the port on the first line
/// and the authentication token on the second line.
fn resolve_agent_info() -> Result<AgentInfo, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let port_file = format!("{}/.cssh/agent_port", home);
    let content = std::fs::read_to_string(&port_file)
        .map_err(|e| format!("failed to read {}: {}", port_file, e))?;
    let mut lines = content.lines();
    let port_str = lines
        .next()
        .ok_or_else(|| format!("empty port file: {}", port_file))?;
    let port = port_str
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("invalid port in {}: '{}'", port_file, port_str.trim()))?;
    let token = lines
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(AgentInfo { port, token })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: cexec <command> [args...]");
        std::process::exit(1);
    }

    let agent_info = match resolve_agent_info() {
        Ok(info) => info,
        Err(e) => {
            eprintln!("cssh-remote error: {}", e);
            std::process::exit(1);
        }
    };

    match client::execute(agent_info.port, agent_info.token, args).await {
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
    fn resolve_agent_info_reads_port_and_token_from_file() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let cssh_dir = fake_home.path().join(".cssh");
        std::fs::create_dir_all(&cssh_dir).unwrap();
        std::fs::write(cssh_dir.join("agent_port"), "54321\nabc123token\n").unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_agent_info();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        let info = result.unwrap();
        assert_eq!(info.port, 54321);
        assert_eq!(info.token, "abc123token");
    }

    #[test]
    fn resolve_agent_info_handles_port_only_file_for_backwards_compatibility() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let cssh_dir = fake_home.path().join(".cssh");
        std::fs::create_dir_all(&cssh_dir).unwrap();
        std::fs::write(cssh_dir.join("agent_port"), "54321\n").unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_agent_info();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        let info = result.unwrap();
        assert_eq!(info.port, 54321);
        assert_eq!(info.token, "");
    }

    #[test]
    fn resolve_agent_info_returns_error_when_file_missing() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_agent_info();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to read"));
    }

    #[test]
    fn resolve_agent_info_returns_error_for_invalid_port() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let cssh_dir = fake_home.path().join(".cssh");
        std::fs::create_dir_all(&cssh_dir).unwrap();
        std::fs::write(cssh_dir.join("agent_port"), "not_a_number\n").unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_agent_info();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid port"));
    }
}
