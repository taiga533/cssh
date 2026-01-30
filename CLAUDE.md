# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Language Policy

All code, comments, documentation, error messages, log messages, and test names in this repository MUST be written in English.

## Commit Message Conventions

This repository enforces **Conventional Commits** format. All commit messages MUST follow this structure:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**Allowed types:**
- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, missing semicolons, etc.)
- `refactor`: Code refactoring without changing functionality
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `build`: Build system or dependency changes
- `ci`: CI/CD configuration changes
- `chore`: Other changes that don't modify src or test files

**Examples:**
```
feat(agent): add timeout support for command execution
fix(cli): convert SSH -p option to SCP -P correctly
docs: update README with installation instructions
test(common): add integration test for multiple sequential commands
refactor(protocol): simplify message encoding logic
```

**Breaking changes:**
Append `!` after type/scope or add `BREAKING CHANGE:` in footer:
```
feat(protocol)!: change message format to use 8-byte length prefix

BREAKING CHANGE: The protocol now uses 8-byte length prefix instead of 4-byte.
Clients must be updated to handle the new format.
```

## Build & Test Commands

```bash
# Build entire workspace
cargo build --workspace

# Release build
cargo build --release --workspace

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p cssh-common
cargo test -p cssh-agent
cargo test -p cssh-remote
cargo test -p cssh-cli

# Run a specific test function (substring match)
cargo test -p cssh-common -- request_serialization

# E2E tests (requires Docker)
bash e2e/run_e2e.sh
```

Build produces three binaries: `cssh` (CLI), `cssh-agent` (host-side agent), `cexec` (remote executable).

## Architecture

A tool that wraps SSH and executes host-side commands from the remote via reverse port forwarding.

### Crate Structure and Dependencies

```
cssh-cli  ──→ cssh-common ←── cssh-agent
                   ↑
               cssh-remote
```

- **cssh-common**: Foundation for all crates. Provides protocol definitions (`ExecuteRequest`/`ExecuteResponse`), error type (`CsshError`), and JSON read/write functions over TCP.
- **cssh-agent**: Host-side TCP server. Receives requests, executes commands via `std::process::Command`, and returns results. Outputs port number to stdout for the parent process (tracing logs go to stderr).
- **cssh-remote**: Remote-side TCP client (binary name: `cexec`). Reads the agent port from the `CSSH_PORT` environment variable.
- **cssh-cli**: CLI entry point (binary name: `cssh`). Orchestrates: agent startup → SCP deployment of cexec → SSH connection (`-R` reverse port forwarding + `SetEnv`). Argument parsing is manual (`args.rs`) to pass SSH options through transparently.

### Communication Protocol

JSON over TCP with a 4-byte big-endian length prefix. Maximum message size is 16MB. `protocol.rs` provides `read_message`/`write_message` (async) and `encode_message`/`decode_message` (sync).

### Key Design Decisions

- Agent port notification uses the first line of stdout. The parent process reads it with `BufReader::read_line`.
- `AgentGuard` (`ssh.rs`) uses the RAII pattern to automatically kill the agent process on drop.
- When deploying via SCP, SSH's `-p` option is converted to SCP's `-P` in `deployment.rs`.
- Dependency versions are centrally managed in the workspace root `[workspace.dependencies]`.

## Test Conventions

- Follow the Arrange - Act - Assert pattern.
- Test names must clearly describe what is being tested (e.g., `echo_command_executes_correctly`).
- Minimize mocks. For TCP tests, bind a real `TcpListener` to `127.0.0.1:0`.
- Integration tests go in `cssh-common/tests/integration_test.rs`.
