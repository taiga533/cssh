# cssh

A CLI tool that wraps SSH and automatically starts a host-side agent on connection. Enables executing host-side commands from the remote via reverse port forwarding.

## Overview

```
┌──────────────┐        SSH (-R)         ┌──────────────┐
│  Host side   │◄────────────────────────│ Remote side  │
│              │                         │              │
│  cssh-agent  │◄───── TCP (JSON) ───────│  cexec       │
│(cmd executor)│                         │(cmd sender)  │
└──────────────┘                         └──────────────┘
```

`cssh` automatically performs the following:

1. Starts `cssh-agent` in the background on the host
2. Deploys the `cexec` binary to `~/.cssh/` on the remote (first time only)
3. Opens an SSH connection with `-R` reverse port forwarding
4. Running `cexec <command>` on the remote executes that command on the host

## Installation

```bash
cargo build --release
```

Build artifacts are generated in `target/release/`:

| Binary | Description |
|---|---|
| `cssh` | CLI entry point |
| `cssh-agent` | Host-side agent |
| `cexec` | Remote executable |

Place all three binaries in the same directory and add it to your PATH.

```bash
export PATH="$(pwd)/target/release:$PATH"
```

## Usage

### Basic

```bash
cssh user@example.com
```

### With SSH options

SSH options (`-p`, `-i`, etc.) are passed through to ssh as-is.

```bash
cssh -p 2222 -i ~/.ssh/id_rsa user@example.com
```

### Specifying a remote command

Use `--` to specify a command to run on the remote.

```bash
cssh user@example.com -- ls -la
```

### Executing host-side commands from the remote

After connecting via SSH, run the following in the remote shell:

```bash
cexec echo "hello from host"
cexec cat /etc/hostname
cexec pwd
```

### cssh-specific options

| Option | Description | Default |
|---|---|---|
| `--remote-name <NAME>` | Remote executable name | `cexec` |
| `--listen-port <PORT>` | Agent port | Auto-assigned |

```bash
cssh --remote-name myexec --listen-port 8080 user@example.com
```

## SSH Server Configuration

To send the `CSSH_PORT` environment variable to the remote, add the following to `sshd_config` on the SSH server:

```
AcceptEnv CSSH_PORT
```

## Communication Protocol

JSON over TCP with a length prefix.

```
[4 bytes: message length (big-endian)][JSON payload]
```

**Request:**

```json
{ "args": ["echo", "hello"] }
```

**Response:**

```json
{ "exit_code": 0, "stdout": [...], "stderr": [...] }
```

## Project Structure

```
cssh/
├── Cargo.toml          # Workspace definition
├── cssh-common/        # Shared library (protocol definitions, error types)
├── cssh-agent/         # Host-side agent (TCP server, command execution)
├── cssh-remote/        # Remote executable (TCP client)
├── cssh-cli/           # cssh CLI (argument parsing, SSH connection, deployment)
└── e2e/                # E2E tests (Docker SSH environment)
```

## Tests

```bash
# Run all tests
cargo test --workspace

# E2E tests (requires Docker)
bash e2e/run_e2e.sh
```

## License

MIT
