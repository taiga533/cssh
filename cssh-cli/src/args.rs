/// Parsed cssh CLI arguments.
#[derive(Debug, Clone)]
pub struct CsshArgs {
    /// Remote executable name.
    pub remote_name: String,
    /// Agent listen port (0 = auto).
    pub listen_port: u16,
    /// SSH option arguments passed through (-p, -i, etc.).
    pub ssh_options: Vec<String>,
    /// Destination (user@host).
    pub destination: String,
    /// Remote command to execute (after --).
    pub remote_command: Vec<String>,
}

/// Parse CLI arguments.
///
/// Format: cssh [--remote-name NAME] [--listen-port PORT] [SSH_OPTIONS...] <destination> [-- <COMMAND...>]
pub fn parse() -> Result<CsshArgs, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    parse_from(&args)
}

/// Parse from a given argument list (for testing).
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
                .ok_or("--remote-name requires a value")?
                .clone();
        } else if arg == "--listen-port" {
            i += 1;
            listen_port = args
                .get(i)
                .ok_or("--listen-port requires a value")?
                .parse()
                .map_err(|_| "--listen-port must be a number")?;
        } else if arg.starts_with('-') {
            // SSH options
            ssh_options.push(arg.clone());
            // -p, -i, -l, -o, etc. take the next argument as a value
            if matches!(arg.as_str(), "-p" | "-i" | "-l" | "-o" | "-F" | "-J" | "-W") {
                i += 1;
                ssh_options.push(
                    args.get(i)
                        .ok_or(format!("{} requires a value", arg))?
                        .clone(),
                );
            }
        } else {
            // Destination
            destination = Some(arg.clone());
        }

        i += 1;
    }

    if found_separator {
        remote_command = args[i..].to_vec();
    }

    let destination = destination.ok_or("destination is required")?;

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
    fn parses_basic_arguments_correctly() {
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
    fn parses_arguments_with_ssh_options() {
        // Arrange
        let args = s(&["-p", "2222", "-i", "~/.ssh/id_rsa", "user@host"]);

        // Act
        let result = parse_from(&args).unwrap();

        // Assert
        assert_eq!(result.destination, "user@host");
        assert_eq!(result.ssh_options, s(&["-p", "2222", "-i", "~/.ssh/id_rsa"]));
    }

    #[test]
    fn parses_cssh_specific_options() {
        // Arrange
        let args = s(&["--remote-name", "myexec", "--listen-port", "8080", "user@host"]);

        // Act
        let result = parse_from(&args).unwrap();

        // Assert
        assert_eq!(result.remote_name, "myexec");
        assert_eq!(result.listen_port, 8080);
    }

    #[test]
    fn parses_remote_command_after_separator() {
        // Arrange
        let args = s(&["user@host", "--", "cat", "file.txt"]);

        // Act
        let result = parse_from(&args).unwrap();

        // Assert
        assert_eq!(result.remote_command, s(&["cat", "file.txt"]));
    }

    #[test]
    fn returns_error_when_destination_is_missing() {
        // Arrange
        let args = s(&["-p", "2222"]);

        // Act
        let result = parse_from(&args);

        // Assert
        assert!(result.is_err());
    }
}
