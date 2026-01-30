/// Remote installation directory.
pub fn remote_install_dir() -> &'static str {
    "~/.cssh"
}

/// Remote binary name.
pub fn remote_binary_name() -> &'static str {
    "cexec"
}

/// Generate a setup script for the remote environment.
///
/// This script is executed on the remote host to:
/// 1. Create the ~/.cssh directory
/// 2. Add it to PATH in ~/.bashrc
pub fn generate_setup_script() -> String {
    let install_dir = remote_install_dir();
    let path_export = format!(r#"export PATH="$HOME/.cssh:$PATH""#);

    // Unix/Linux shell script
    format!(
        r#"mkdir -p {} && grep -q '{}' ~/.bashrc 2>/dev/null || echo '{}' >> ~/.bashrc"#,
        install_dir, path_export, path_export
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_install_dir_returns_correct_value() {
        assert_eq!(remote_install_dir(), "~/.cssh");
    }

    #[test]
    fn remote_binary_name_returns_correct_value() {
        assert_eq!(remote_binary_name(), "cexec");
    }

    #[test]
    fn generate_setup_script_creates_valid_script() {
        let script = generate_setup_script();
        assert!(script.contains("mkdir -p"));
        assert!(script.contains("~/.cssh"));
        assert!(script.contains("export PATH"));
        assert!(script.contains(".bashrc"));
    }
}
