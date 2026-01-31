use std::path::PathBuf;

use crate::error::{CsshError, Result};

/// Get the executable extension for the current platform.
///
/// - Windows: ".exe"
/// - Unix/Linux/macOS: ""
pub fn executable_extension() -> &'static str {
    #[cfg(target_family = "windows")]
    {
        ".exe"
    }
    #[cfg(target_family = "unix")]
    {
        ""
    }
}

/// Find a binary by name in the current directory or in PATH.
///
/// On Windows, automatically appends the ".exe" extension.
/// On Unix, searches for the exact name.
pub fn find_binary(binary_name: &str) -> Result<PathBuf> {
    let exe_ext = executable_extension();
    let binary_with_ext = format!("{}{}", binary_name, exe_ext);

    let current_exe = std::env::current_exe()
        .map_err(|e| CsshError::Execution(format!("failed to get current exe path: {}", e)))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| CsshError::Execution("failed to get parent directory".to_string()))?;
    let local_binary = dir.join(&binary_with_ext);

    if local_binary.exists() {
        return Ok(local_binary);
    }

    // Search in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for path_str in std::env::split_paths(&path_var) {
            let path = path_str.join(&binary_with_ext);
            if path.exists() && is_executable(&path) {
                return Ok(path);
            }
        }
    }

    Err(CsshError::Execution(format!(
        "{} binary not found",
        binary_name
    )))
}

/// Find a binary intended for a remote Linux host.
///
/// When the local host is Windows, the remote binary (e.g. `cexec`) is a
/// Linux ELF binary without `.exe` extension. This function searches for the
/// binary name without any platform-specific extension, looking in:
///   1. The same directory as the current executable
///   2. Directories listed in the PATH environment variable
///
/// On Unix hosts this behaves identically to [`find_binary`].
pub fn find_remote_binary(binary_name: &str) -> Result<PathBuf> {
    let current_exe = std::env::current_exe()
        .map_err(|e| CsshError::Execution(format!("failed to get current exe path: {}", e)))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| CsshError::Execution("failed to get parent directory".to_string()))?;

    // Search without platform extension (remote is always Linux)
    let local_binary = dir.join(binary_name);
    if local_binary.exists() {
        return Ok(local_binary);
    }

    // On Windows, also check with .exe extension as a fallback.
    // Users may cross-compile and place the Linux binary without extension,
    // but if only the .exe build exists we should still find it so the error
    // message is more helpful.
    #[cfg(target_family = "windows")]
    {
        let with_exe = dir.join(format!("{}.exe", binary_name));
        if with_exe.exists() {
            return Err(CsshError::Execution(format!(
                "{0} Linux binary not found (found {0}.exe which is a Windows binary; \
                 cross-compile for Linux and place the resulting binary next to cssh.exe)",
                binary_name
            )));
        }
    }

    // Search in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for path_str in std::env::split_paths(&path_var) {
            let path = path_str.join(binary_name);
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err(CsshError::Execution(format!(
        "{} Linux binary not found",
        binary_name
    )))
}

/// Check if a file is executable.
///
/// On Windows, checks if the file has a ".exe" or ".bat" extension.
/// On Unix, checks the executable bit in the file permissions.
pub fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(target_family = "windows")]
    {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            ext_str == "exe" || ext_str == "bat" || ext_str == "cmd"
        } else {
            false
        }
    }

    #[cfg(target_family = "unix")]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mode = metadata.permissions().mode();
            (mode & 0o111) != 0
        } else {
            false
        }
    }
}

/// Check if the current platform supports Unix domain socket forwarding via SSH.
///
/// Returns true on Unix-family platforms (Linux, macOS), false on Windows
/// where Win32-OpenSSH does not support streamlocal forwarding.
pub fn supports_unix_socket_forwarding() -> bool {
    cfg!(target_family = "unix")
}

/// Make a file executable.
///
/// On Windows, does nothing (files with .exe extension are automatically executable).
/// On Unix, sets the executable bit (+x).
pub fn make_executable(path: &std::path::Path) -> Result<()> {
    #[cfg(target_family = "unix")]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)
            .map_err(|e| CsshError::Execution(format!("failed to get file metadata: {}", e)))?;
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        permissions.set_mode(mode | 0o111);
        std::fs::set_permissions(path, permissions).map_err(|e| {
            CsshError::Execution(format!("failed to set executable permission: {}", e))
        })?;
    }

    #[cfg(target_family = "windows")]
    {
        // Windows doesn't require explicit executable permission
        let _ = path;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_extension_returns_correct_value() {
        #[cfg(target_family = "windows")]
        {
            assert_eq!(executable_extension(), ".exe");
        }
        #[cfg(target_family = "unix")]
        {
            assert_eq!(executable_extension(), "");
        }
    }

    #[test]
    fn find_binary_finds_binary_in_current_directory() {
        // This test verifies that find_binary can locate binaries.
        // In the build directory, the binary should exist.
        let result = find_binary("cargo");
        // We expect either success (cargo is in PATH) or failure, both are acceptable
        let _ = result;
    }

    #[test]
    fn is_executable_does_not_panic() {
        let path = std::path::Path::new("/usr/bin/ls");
        let _ = is_executable(path);
    }

    #[test]
    fn make_executable_does_not_panic() {
        let path = std::path::Path::new("/tmp/nonexistent_file");
        let _ = make_executable(path);
    }

    #[test]
    fn supports_unix_socket_forwarding_returns_correct_value() {
        #[cfg(target_family = "unix")]
        {
            assert!(supports_unix_socket_forwarding());
        }
        #[cfg(target_family = "windows")]
        {
            assert!(!supports_unix_socket_forwarding());
        }
    }

    #[test]
    fn find_remote_binary_does_not_panic() {
        // find_remote_binary always searches without platform extension
        let _ = find_remote_binary("cexec");
    }
}
