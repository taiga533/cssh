# Authentication Token for Agent Communication

## Goal

Add shared secret authentication to prevent unauthorized command execution via the forwarded TCP port on shared remote hosts.

## Design

### Token Flow

```
cssh (host)
  ├─ Generate random 32-byte hex token (64 chars)
  ├─ Pass token to cssh-agent via CLI arg (argv[2])
  ├─ Write "{port}\n{token}\n" to ~/.cssh/agent_port (chmod 600)
  └─ SSH -R {port}:127.0.0.1:{port}

cssh-agent
  ├─ Receive token via argv[2]
  ├─ On each request: validate request.token == expected
  └─ Reject with error response + close on mismatch

cexec (remote)
  ├─ Read ~/.cssh/agent_port → parse port + token
  ├─ Set token field on ExecuteRequest
  └─ Send request
```

## Changes by File

### 1. `cssh-common/src/protocol.rs`
- Add `token` field to `ExecuteRequest`:
  ```rust
  pub struct ExecuteRequest {
      pub args: Vec<String>,
      #[serde(default)]
      pub token: String,
  }
  ```

### 2. `cssh-common/src/error.rs`
- Add variant:
  ```rust
  #[error("Authentication failed")]
  AuthenticationFailed,
  ```

### 3. `cssh-agent/src/main.rs`
- Parse optional token from `argv[2]`
- Pass to `server::run(port, token)`

### 4. `cssh-agent/src/server.rs`
- `run()` accepts `Option<String>` token parameter
- `handle_connection()` validates `request.token` on each request
- Mismatch → error response + close connection

### 5. `cssh-cli/src/ssh.rs`
- `generate_token()`: 32 random bytes → hex string using `getrandom` crate
- `start_agent()`: pass token as second arg
- `write_remote_port_file()`: write `"{port}\n{token}\n"`, chmod 600

### 6. `cssh-remote/src/main.rs`
- `resolve_port()` → `resolve_agent_info()` returning `(u16, String)` (port, token)
- Parse two lines from agent_port file

### 7. `cssh-remote/src/client.rs`
- `execute(port, token, args)`: set token on request

## Dependencies

- Add `getrandom = "0.3"` to workspace and cssh-cli (already transitive via tempfile)

## Tests

- `cssh-common`: ExecuteRequest serialization with/without token (`#[serde(default)]` compat)
- `cssh-agent/server.rs`: reject wrong token, accept correct token
- `cssh-remote/main.rs`: `resolve_agent_info()` parses port+token
- `cssh-remote/client.rs`: token sent in request
- Integration test: full flow with token

## Verification

```bash
cargo build --workspace
cargo test --workspace
```
