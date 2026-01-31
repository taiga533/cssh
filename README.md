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
2. Opens an SSH connection with `-R` reverse port forwarding
3. Running `cexec <command>` on the remote executes that command on the host

## Installation

### Host side (where you run `cssh`)

Install `cssh` and `cssh-agent`:

```bash
cargo install --git https://github.com/taiga533/cssh cssh-cli cssh-agent
```

`cssh-agent` must be in your PATH so that `cssh` can start it automatically.

**Windows notes:**

- Windows 10/11 is supported as the host.
- OpenSSH Client is required (included by default in Windows 10 1809+). Verify with `ssh -V`.
- One of the following C toolchains is required:
  - **Visual Studio Build Tools (MSVC)** — Download [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/) and select "Desktop development with C++" during installation.
  - **MinGW (GNU)** — Install via [Scoop](https://scoop.sh/) (`scoop install mingw`) or [Chocolatey](https://chocolatey.org/) (`choco install mingw`), then run `rustup default stable-gnu`.

### Remote side (where you run `cexec`)

Install `cexec` on each remote host:

```bash
cargo install --git https://github.com/taiga533/cssh cssh-remote
```

`cexec` must be in your PATH on the remote host.

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
| `--listen-port <PORT>` | Agent port | Auto-assigned |

```bash
cssh --listen-port 8080 user@example.com
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
