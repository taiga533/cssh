use std::sync::Arc;

use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::executor;

/// Maximum consecutive authentication failures before applying a delay.
const MAX_FAILURES_BEFORE_DELAY: u32 = 3;

/// Delay in seconds applied after exceeding the failure threshold.
const DELAY_SECONDS: u64 = 10;

/// Tracks consecutive authentication failures to throttle brute-force attempts.
struct BruteForceGuard {
    consecutive_failures: u32,
}

impl BruteForceGuard {
    /// Create a new guard with zero failures.
    fn new() -> Self {
        Self {
            consecutive_failures: 0,
        }
    }

    /// Record an authentication failure and return whether a delay should be applied.
    fn record_failure(&mut self) -> bool {
        self.consecutive_failures += 1;
        self.consecutive_failures >= MAX_FAILURES_BEFORE_DELAY
    }

    /// Reset the failure counter on successful authentication.
    fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }
}

/// Compare two token strings in constant time to prevent timing attacks.
fn constant_time_token_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    a_bytes.ct_eq(b_bytes).into()
}

/// Start the agent server on the specified port with optional token authentication.
pub async fn run(port: u16, token: Option<String>) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let local_addr = listener.local_addr()?;
    // Output port number to stdout (read by parent process; must precede log output)
    println!("{}", local_addr.port());
    info!("agent started: {}", local_addr);

    let token = token.map(Arc::new);
    let guard = Arc::new(Mutex::new(BruteForceGuard::new()));

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("connection accepted: {}", addr);

        // Apply delay if brute-force threshold has been reached
        {
            let g = guard.lock().await;
            if g.consecutive_failures >= MAX_FAILURES_BEFORE_DELAY {
                warn!(
                    "brute-force delay: {} consecutive failures",
                    g.consecutive_failures
                );
                tokio::time::sleep(std::time::Duration::from_secs(DELAY_SECONDS)).await;
            }
        }

        let token = token.clone();
        let guard = guard.clone();
        tokio::spawn(async move {
            if let Err(e) =
                handle_connection(stream, token.as_ref().map(|s| s.as_str()), &guard).await
            {
                error!("connection handler error: {}", e);
            }
        });
    }
}

/// Handle a single connection with optional token validation.
async fn handle_connection(
    stream: tokio::net::TcpStream,
    expected_token: Option<&str>,
    guard: &Mutex<BruteForceGuard>,
) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    loop {
        let request: ExecuteRequest = match read_message(&mut reader).await {
            Ok(req) => req,
            Err(cssh_common::error::CsshError::Io(ref e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset
                ) =>
            {
                info!("client disconnected");
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        // Validate authentication token
        if let Some(expected) = expected_token {
            if !constant_time_token_eq(&request.token, expected) {
                warn!("authentication failed: invalid token");
                {
                    let mut g = guard.lock().await;
                    g.record_failure();
                }
                let response = ExecuteResponse {
                    exit_code: 1,
                    stdout: Vec::new(),
                    stderr: b"authentication failed".to_vec(),
                };
                write_message(&mut writer, &response).await?;
                return Err(cssh_common::error::CsshError::AuthenticationFailed);
            } else {
                let mut g = guard.lock().await;
                g.record_success();
            }
        }

        info!("executing command: {:?}", request.args);
        let response = executor::execute(&request).unwrap_or_else(|e| {
            error!("command execution failed: {}", e);
            ExecuteResponse {
                exit_code: 1,
                stdout: Vec::new(),
                stderr: b"command execution failed".to_vec(),
            }
        });

        write_message(&mut writer, &response).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssh_common::protocol::write_message as client_write;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn server_executes_command_and_returns_response() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let guard = Arc::new(Mutex::new(BruteForceGuard::new()));

        let g = guard.clone();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, None, &g).await.unwrap();
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "test".to_string()],
            token: String::new(),
        };

        // Act
        client_write(&mut writer, &request).await.unwrap();
        let response: ExecuteResponse = read_message(&mut reader).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&response.stdout).trim(), "test");
    }

    #[tokio::test]
    async fn server_terminates_gracefully_on_client_disconnect() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let guard = Arc::new(Mutex::new(BruteForceGuard::new()));

        let g = guard.clone();
        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, None, &g).await
        });

        // Act
        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        drop(stream); // disconnect immediately

        // Assert
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn server_accepts_request_with_correct_token() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let token = "valid_token_123";
        let guard = Arc::new(Mutex::new(BruteForceGuard::new()));

        let expected = token.to_string();
        let g = guard.clone();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, Some(&expected), &g).await.ok();
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "authenticated".to_string()],
            token: token.to_string(),
        };

        // Act
        client_write(&mut writer, &request).await.unwrap();
        let response: ExecuteResponse = read_message(&mut reader).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(
            String::from_utf8_lossy(&response.stdout).trim(),
            "authenticated"
        );
    }

    #[tokio::test]
    async fn server_rejects_request_with_wrong_token() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let guard = Arc::new(Mutex::new(BruteForceGuard::new()));

        let g = guard.clone();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = handle_connection(stream, Some("correct_token"), &g).await;
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "should_fail".to_string()],
            token: "wrong_token".to_string(),
        };

        // Act
        client_write(&mut writer, &request).await.unwrap();
        let response: ExecuteResponse = read_message(&mut reader).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 1);
        assert_eq!(
            String::from_utf8_lossy(&response.stderr),
            "authentication failed"
        );
    }

    #[tokio::test]
    async fn brute_force_guard_records_failures_and_resets_on_success() {
        // Arrange
        let mut guard = BruteForceGuard::new();

        // Act & Assert - failures accumulate
        assert!(!guard.record_failure()); // 1st failure
        assert!(!guard.record_failure()); // 2nd failure
        assert!(guard.record_failure()); // 3rd failure triggers delay

        // Act - success resets counter
        guard.record_success();

        // Assert - counter is reset, first failure does not trigger delay
        assert!(!guard.record_failure());
    }

    #[tokio::test]
    async fn brute_force_delay_is_applied_after_threshold() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let guard = Arc::new(Mutex::new(BruteForceGuard::new()));

        // Pre-fill failures to trigger delay
        {
            let mut g = guard.lock().await;
            for _ in 0..MAX_FAILURES_BEFORE_DELAY {
                g.record_failure();
            }
        }

        let g = guard.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let guard_inner = g.clone();

                // Apply delay like the real server
                {
                    let g2 = guard_inner.lock().await;
                    if g2.consecutive_failures >= MAX_FAILURES_BEFORE_DELAY {
                        tokio::time::sleep(std::time::Duration::from_secs(DELAY_SECONDS)).await;
                    }
                }

                let gi = guard_inner.clone();
                tokio::spawn(async move {
                    let _ = handle_connection(stream, Some("token"), &gi).await;
                });
            }
        });

        // Act - measure connection handling time
        let start = std::time::Instant::now();
        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();
        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "test".to_string()],
            token: "token".to_string(),
        };
        client_write(&mut writer, &request).await.unwrap();
        let _response: ExecuteResponse = read_message(&mut reader).await.unwrap();
        let elapsed = start.elapsed();

        // Assert - should have been delayed by at least DELAY_SECONDS
        assert!(
            elapsed.as_secs() >= DELAY_SECONDS,
            "expected delay of at least {}s, got {:?}",
            DELAY_SECONDS,
            elapsed
        );
    }

    #[tokio::test]
    async fn execution_error_does_not_leak_details_to_remote() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let guard = Arc::new(Mutex::new(BruteForceGuard::new()));

        let g = guard.clone();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, None, &g).await.unwrap();
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let request = ExecuteRequest {
            args: vec!["/nonexistent/binary/xyz".to_string()],
            token: String::new(),
        };

        // Act
        client_write(&mut writer, &request).await.unwrap();
        let response: ExecuteResponse = read_message(&mut reader).await.unwrap();

        // Assert
        let stderr = String::from_utf8_lossy(&response.stderr);
        assert_eq!(stderr, "command execution failed");
        assert!(
            !stderr.contains("/nonexistent"),
            "error should not leak system paths"
        );
        assert_eq!(response.exit_code, 1);
    }

    #[test]
    fn constant_time_token_eq_compares_correctly() {
        // Arrange & Act & Assert
        assert!(constant_time_token_eq("abc", "abc"));
        assert!(!constant_time_token_eq("abc", "def"));
        assert!(!constant_time_token_eq("abc", "abcd"));
        assert!(!constant_time_token_eq("", "a"));
        assert!(constant_time_token_eq("", ""));
    }
}
