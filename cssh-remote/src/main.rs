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
    let port_file = std::path::PathBuf::from(home).join(".cssh").join("agent_port");
    let content = std::fs::read_to_string(&port_file)
        .map_err(|e| format!("failed to read {}: {}", port_file.display(), e))?;
    parse_agent_info(&content, &port_file.to_string_lossy())
}

/// Parse agent info from the content of the port file.
///
/// The first line contains the port number, and the optional second line
/// contains the authentication token.
fn parse_agent_info(content: &str, path: &str) -> Result<AgentInfo, String> {
    let mut lines = content.lines();
    let port_str = lines
        .next()
        .ok_or_else(|| format!("empty port file: {}", path))?;
    let port = port_str
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("invalid port in {}: '{}'", path, port_str.trim()))?;
    let token = lines.next().unwrap_or("").trim().to_string();
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

    #[test]
    fn parse_agent_info_reads_port_and_token() {
        // Arrange
        let content = "54321\nabc123token\n";

        // Act
        let info = parse_agent_info(content, "test").unwrap();

        // Assert
        assert_eq!(info.port, 54321);
        assert_eq!(info.token, "abc123token");
    }

    #[test]
    fn parse_agent_info_handles_port_only_for_backwards_compatibility() {
        // Arrange
        let content = "54321\n";

        // Act
        let info = parse_agent_info(content, "test").unwrap();

        // Assert
        assert_eq!(info.port, 54321);
        assert_eq!(info.token, "");
    }

    #[test]
    fn parse_agent_info_returns_error_for_empty_content() {
        // Arrange
        let content = "";

        // Act
        let result = parse_agent_info(content, "test");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty port file"));
    }

    #[test]
    fn parse_agent_info_returns_error_for_invalid_port() {
        // Arrange
        let content = "not_a_number\n";

        // Act
        let result = parse_agent_info(content, "test");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid port"));
    }

    #[test]
    fn parse_agent_info_trims_whitespace() {
        // Arrange
        let content = "  12345  \n  mytoken  \n";

        // Act
        let info = parse_agent_info(content, "test").unwrap();

        // Assert
        assert_eq!(info.port, 12345);
        assert_eq!(info.token, "mytoken");
    }
}
