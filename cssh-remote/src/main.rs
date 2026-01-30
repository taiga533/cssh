mod client;

use std::path::PathBuf;

/// Resolve the agent connection by scanning the socket directory.
///
/// Scans `~/.cssh/sockets/` for `.sock` files and returns the first
/// one that accepts a connection.
fn resolve_socket() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let sockets_dir = format!("{}/.cssh/sockets", home);
    if let Ok(entries) = std::fs::read_dir(&sockets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sock") {
                if std::os::unix::net::UnixStream::connect(&path).is_ok() {
                    return Ok(path);
                }
            }
        }
    }

    Err("no active cssh agent found (no sockets in ~/.cssh/sockets/)".to_string())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: cexec <command> [args...]");
        std::process::exit(1);
    }

    let socket_path = match resolve_socket() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cssh-remote error: {}", e);
            std::process::exit(1);
        }
    };

    match client::execute(&socket_path, args).await {
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
    use std::os::unix::net::UnixListener;
    use tempfile::TempDir;

    #[test]
    fn resolve_socket_finds_connectable_socket() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let sock_path = tmp.path().join("12345.sock");
        let _listener = UnixListener::bind(&sock_path).unwrap();

        // Temporarily override HOME
        let original_home = std::env::var("HOME").unwrap();
        // Create the sockets dir structure under tmp
        let fake_home = TempDir::new().unwrap();
        let sockets_dir = fake_home.path().join(".cssh/sockets");
        std::fs::create_dir_all(&sockets_dir).unwrap();
        let sock_in_dir = sockets_dir.join("12345.sock");
        let _listener2 = UnixListener::bind(&sock_in_dir).unwrap();

        std::env::set_var("HOME", fake_home.path().to_str().unwrap());
        // Act
        let result = resolve_socket();
        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap().to_str().unwrap().ends_with(".sock"));
    }

    #[test]
    fn resolve_socket_returns_error_when_no_sockets() {
        // Arrange
        let fake_home = TempDir::new().unwrap();
        let sockets_dir = fake_home.path().join(".cssh/sockets");
        std::fs::create_dir_all(&sockets_dir).unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", fake_home.path().to_str().unwrap());

        // Act
        let result = resolve_socket();

        // Restore
        std::env::set_var("HOME", &original_home);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no active cssh agent found"));
    }
}
