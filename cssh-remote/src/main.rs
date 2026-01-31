mod client;

/// Connection info for reaching the agent.
#[derive(Debug, PartialEq)]
pub(crate) enum ConnectionInfo {
    /// Connect via Unix domain socket (used when host is Unix).
    UnixSocket { path: String, token: String },
    /// Connect via TCP (used when host is Windows).
    Tcp { port: u16, token: String },
}

/// Resolve agent connection info by reading the port file.
///
/// Reads `~/.cssh/agent_port` which contains connection info in one of:
/// - `socket|<path>|<token>` (Unix socket)
/// - `tcp|<port>|<token>` (TCP)
fn resolve_connection_info() -> Result<ConnectionInfo, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let port_file = std::path::PathBuf::from(home).join(".cssh").join("agent_port");
    let content = std::fs::read_to_string(&port_file)
        .map_err(|e| format!("failed to read {}: {}", port_file.display(), e))?;
    parse_connection_info(&content, &port_file.to_string_lossy())
}

/// Parse connection info from the content of the port file.
///
/// Supports two formats:
/// - `socket|<path>|<token>` → UnixSocket
/// - `tcp|<port>|<token>` → Tcp
fn parse_connection_info(content: &str, path: &str) -> Result<ConnectionInfo, String> {
    let first_line = content
        .lines()
        .next()
        .ok_or_else(|| format!("empty port file: {}", path))?
        .trim();

    let parts: Vec<&str> = first_line.splitn(3, '|').collect();
    if parts.len() < 3 {
        return Err(format!("invalid connection info in {}: '{}'", path, first_line));
    }
    let kind = parts[0];
    let value = parts[1];
    let token = parts[2].to_string();

    match kind {
        "socket" => Ok(ConnectionInfo::UnixSocket {
            path: value.to_string(),
            token,
        }),
        "tcp" => {
            let port = value
                .parse::<u16>()
                .map_err(|_| format!("invalid port in {}: '{}'", path, value))?;
            Ok(ConnectionInfo::Tcp { port, token })
        }
        _ => Err(format!("unknown connection type in {}: '{}'", path, kind)),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: cexec <command> [args...]");
        std::process::exit(1);
    }

    let conn_info = match resolve_connection_info() {
        Ok(info) => info,
        Err(e) => {
            eprintln!("cssh-remote error: {}", e);
            std::process::exit(1);
        }
    };

    match client::execute(conn_info, args).await {
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
    fn parse_connection_info_parses_socket_format() {
        // Arrange
        let content = "socket|/tmp/cssh-agent-12345.sock|abctoken\n";

        // Act
        let info = parse_connection_info(content, "test").unwrap();

        // Assert
        assert_eq!(
            info,
            ConnectionInfo::UnixSocket {
                path: "/tmp/cssh-agent-12345.sock".to_string(),
                token: "abctoken".to_string(),
            }
        );
    }

    #[test]
    fn parse_connection_info_parses_tcp_format() {
        // Arrange
        let content = "tcp|54321|mytoken\n";

        // Act
        let info = parse_connection_info(content, "test").unwrap();

        // Assert
        assert_eq!(
            info,
            ConnectionInfo::Tcp {
                port: 54321,
                token: "mytoken".to_string(),
            }
        );
    }

    #[test]
    fn parse_connection_info_returns_error_for_empty_content() {
        // Arrange
        let content = "";

        // Act
        let result = parse_connection_info(content, "test");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty port file"));
    }

    #[test]
    fn parse_connection_info_returns_error_for_invalid_port_in_tcp_format() {
        // Arrange
        let content = "tcp|bad|token\n";

        // Act
        let result = parse_connection_info(content, "test");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid port"));
    }

    #[test]
    fn parse_connection_info_returns_error_for_unknown_type() {
        // Arrange
        let content = "unknown|value|token\n";

        // Act
        let result = parse_connection_info(content, "test");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown connection type"));
    }

    #[test]
    fn parse_connection_info_returns_error_for_legacy_format() {
        // Arrange
        let content = "54321\nabc123token\n";

        // Act
        let result = parse_connection_info(content, "test");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid connection info"));
    }
}
