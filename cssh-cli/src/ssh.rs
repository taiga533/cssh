use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use crate::args::CsshArgs;
use crate::deployment;
use cssh_common::platform;

/// Run the main cssh workflow.
///
/// 1. Generate authentication token
/// 2. Start the agent in the background
/// 3. Deploy cssh-remote to the remote host
/// 4. Write agent port and token to a file on the remote host
/// 5. Open SSH connection with reverse port forwarding
pub async fn run(args: CsshArgs) -> Result<(), String> {
    // Generate authentication token
    let token = generate_token();

    // Start the agent
    let (agent_process, agent_port) = start_agent(&args, &token)?;
    tracing::info!("agent started: port {}", agent_port);

    // Deploy remote binary
    if let Err(e) = deployment::deploy_remote_binary(&args) {
        tracing::warn!("remote binary deployment skipped: {}", e);
    }

    // Write agent port and token to file on remote
    write_remote_port_file(&args, agent_port, &token)?;

    // SSH connection
    let exit_code = run_ssh(&args, agent_port)?;

    // Stop the agent
    drop(agent_process);

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

/// Generate a random 32-byte hex authentication token (64 characters).
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("failed to generate random token");
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Start the agent process in the background and return the assigned port.
fn start_agent(args: &CsshArgs, token: &str) -> Result<(AgentGuard, u16), String> {
    let port_arg = args.listen_port.to_string();

    // Find cssh-agent in the same directory
    let agent_bin = find_agent_binary()?;

    let mut child = Command::new(&agent_bin)
        .arg(&port_arg)
        .arg(token)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start agent: {}", e))?;

    // Read the port number output by the agent
    let stdout = child.stdout.take().ok_or("failed to get agent stdout")?;
    let mut reader = BufReader::new(stdout);
    let mut port_line = String::new();
    reader
        .read_line(&mut port_line)
        .map_err(|e| format!("failed to read port: {}", e))?;

    let port: u16 = port_line
        .trim()
        .parse()
        .map_err(|_| format!("invalid port number: '{}'", port_line.trim()))?;

    Ok((AgentGuard(child), port))
}

/// Find the agent binary.
fn find_agent_binary() -> Result<String, String> {
    platform::find_binary("cssh-agent")
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("{}", e))
}

/// Run SSH connection and return the exit code.
fn run_ssh(args: &CsshArgs, agent_port: u16) -> Result<i32, String> {
    let mut cmd = Command::new("ssh");

    // Reverse port forwarding: connect the remote port to the local agent port
    cmd.arg("-R")
        .arg(format!("{}:127.0.0.1:{}", agent_port, agent_port));

    // User-specified SSH options
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }

    // Destination
    cmd.arg(&args.destination);

    // Remote command
    if !args.remote_command.is_empty() {
        for c in &args.remote_command {
            cmd.arg(c);
        }
    }

    let status = cmd
        .status()
        .map_err(|e| format!("failed to run ssh: {}", e))?;

    Ok(status.code().unwrap_or(1))
}

/// Write the agent port number and token to a file on the remote host.
///
/// Creates `~/.cssh/agent_port` containing the port and token (one per line)
/// with permissions 600 so that cexec can discover the agent securely.
fn write_remote_port_file(args: &CsshArgs, port: u16, token: &str) -> Result<(), String> {
    let mut cmd = Command::new("ssh");
    for opt in &args.ssh_options {
        cmd.arg(opt);
    }
    cmd.arg(&args.destination).arg(format!(
        "mkdir -p ~/.cssh && printf '%s\\n%s\\n' '{}' '{}' > ~/.cssh/agent_port && chmod 600 ~/.cssh/agent_port",
        port, token
    ));
    let status = cmd
        .status()
        .map_err(|e| format!("failed to write port file: {}", e))?;
    if !status.success() {
        return Err("failed to write ~/.cssh/agent_port on remote".to_string());
    }
    Ok(())
}

/// Guard that automatically kills the agent process on drop.
struct AgentGuard(Child);

impl Drop for AgentGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_agent_binary_does_not_panic() {
        // Verify that find_agent_binary does not panic
        let _ = find_agent_binary();
    }

    #[test]
    fn generate_token_returns_64_char_hex_string() {
        // Act
        let token = generate_token();

        // Assert
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_produces_unique_values() {
        // Act
        let token1 = generate_token();
        let token2 = generate_token();

        // Assert
        assert_ne!(token1, token2);
    }
}
