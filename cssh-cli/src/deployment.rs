use std::process::Command;

use crate::args::CsshArgs;
use crate::remote_setup;
use cssh_common::platform;

/// Deploy the cssh-remote binary to the remote host.
///
/// 1. Check if the binary already exists at ~/.cssh/ on the remote
/// 2. Deploy via SCP only if it does not exist
pub fn deploy_remote_binary(args: &CsshArgs) -> Result<(), String> {
    let install_dir = remote_setup::remote_install_dir();
    let remote_bin_path = format!("{}/{}", install_dir, args.remote_name);

    // Check if the binary exists on the remote
    if check_remote_binary_exists(args, &remote_bin_path)? {
        tracing::info!("remote binary already exists");
        return Ok(());
    }

    // Find the local cexec binary path
    let local_bin = find_local_binary()?;

    // Create ~/.cssh/ directory on the remote
    create_remote_directory(args)?;

    // Deploy via SCP
    scp_to_remote(args, &local_bin, &remote_bin_path)?;

    // Set executable permission
    set_remote_executable(args, &remote_bin_path)?;

    tracing::info!("remote binary deployed");
    Ok(())
}

/// Check if the binary exists on the remote.
fn check_remote_binary_exists(args: &CsshArgs, remote_path: &str) -> Result<bool, String> {
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination)
        .arg(format!("test -x {}", remote_path));

    let status = cmd
        .status()
        .map_err(|e| format!("failed to run ssh: {}", e))?;
    Ok(status.success())
}

/// Find the local cexec binary for deployment to a remote Linux host.
///
/// On Windows, searches for the Linux cross-compiled binary (without .exe).
/// On Unix, searches with the normal platform extension.
fn find_local_binary() -> Result<String, String> {
    platform::find_remote_binary("cexec")
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("{}", e))
}

/// Create the remote installation directory.
fn create_remote_directory(args: &CsshArgs) -> Result<(), String> {
    let script = remote_setup::generate_setup_script();
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination).arg(&script);

    let status = cmd
        .status()
        .map_err(|e| format!("failed to create remote directory: {}", e))?;

    if !status.success() {
        return Err("failed to create remote directory".to_string());
    }
    Ok(())
}

/// Send a file to the remote via SCP.
fn scp_to_remote(args: &CsshArgs, local_path: &str, remote_path: &str) -> Result<(), String> {
    let mut cmd = Command::new("scp");
    // Convert SSH options for SCP (-p -> -P)
    let mut iter = args.ssh_options.iter();
    while let Some(opt) = iter.next() {
        if opt == "-p" {
            cmd.arg("-P");
            if let Some(val) = iter.next() {
                cmd.arg(val);
            }
        } else {
            cmd.arg(opt);
        }
    }
    cmd.arg(local_path)
        .arg(format!("{}:{}", args.destination, remote_path));

    let status = cmd
        .status()
        .map_err(|e| format!("failed to run scp: {}", e))?;

    if !status.success() {
        return Err("SCP file transfer failed".to_string());
    }
    Ok(())
}

/// Set executable permission on a remote file.
fn set_remote_executable(args: &CsshArgs, remote_path: &str) -> Result<(), String> {
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination)
        .arg(format!("chmod +x {}", remote_path));

    let status = cmd
        .status()
        .map_err(|e| format!("failed to run chmod: {}", e))?;

    if !status.success() {
        return Err("failed to set executable permission".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_local_binary_does_not_panic() {
        // Act
        // In test environments, cexec may or may not exist in the build directory.
        // This test verifies that find_local_binary does not panic.
        let result = find_local_binary();

        // Assert - success if built, error otherwise (both are OK)
        let _ = result;
    }
}
