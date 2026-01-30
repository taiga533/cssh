use std::sync::Arc;

use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::executor;

/// Start the agent server on the specified port with optional token authentication.
pub async fn run(port: u16, token: Option<String>) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let local_addr = listener.local_addr()?;
    // Output port number to stdout (read by parent process; must precede log output)
    println!("{}", local_addr.port());
    info!("agent started: {}", local_addr);

    let token = token.map(Arc::new);

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("connection accepted: {}", addr);
        let token = token.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, token.as_ref().map(|s| s.as_str())).await {
                error!("connection handler error: {}", e);
            }
        });
    }
}

/// Handle a single connection with optional token validation.
async fn handle_connection(
    stream: tokio::net::TcpStream,
    expected_token: Option<&str>,
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
            if request.token != expected {
                warn!("authentication failed: invalid token");
                let response = ExecuteResponse {
                    exit_code: 1,
                    stdout: Vec::new(),
                    stderr: b"authentication failed".to_vec(),
                };
                write_message(&mut writer, &response).await?;
                return Err(cssh_common::error::CsshError::AuthenticationFailed);
            }
        }

        info!("executing command: {:?}", request.args);
        let response = executor::execute(&request).unwrap_or_else(|e| ExecuteResponse {
            exit_code: 1,
            stdout: Vec::new(),
            stderr: format!("execution error: {}", e).into_bytes(),
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

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, None).await.unwrap();
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

        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, None).await
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

        let expected = token.to_string();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, Some(&expected)).await.ok();
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

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = handle_connection(stream, Some("correct_token")).await;
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
}
