/// Remote binary installation directory.
///
/// Uses `~/.local/bin` which is included in PATH by default on most
/// Linux distributions (XDG standard), avoiding the need to modify
/// shell configuration files like `.bashrc`.
pub fn remote_install_dir() -> &'static str {
    "~/.local/bin"
}

/// Remote binary name.
#[allow(dead_code)]
pub fn remote_binary_name() -> &'static str {
    "cexec"
}

/// Generate a setup script for the remote environment.
///
/// Creates the necessary directories on the remote host:
/// - `~/.local/bin` for the cexec binary
/// - `~/.cssh` for agent port file
pub fn generate_setup_script() -> String {
    format!("mkdir -p {} ~/.cssh", remote_install_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_install_dir_returns_correct_value() {
        assert_eq!(remote_install_dir(), "~/.local/bin");
    }

    #[test]
    fn remote_binary_name_returns_correct_value() {
        assert_eq!(remote_binary_name(), "cexec");
    }

    #[test]
    fn generate_setup_script_creates_valid_script() {
        let script = generate_setup_script();
        assert!(script.contains("mkdir -p"));
        assert!(script.contains("~/.local/bin"));
        assert!(script.contains("~/.cssh"));
        assert!(!script.contains(".bashrc"));
    }
}
