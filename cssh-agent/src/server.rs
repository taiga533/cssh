use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::executor;

/// Start the agent server on the specified port.
pub async fn run(port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let local_addr = listener.local_addr()?;
    // Output port number to stdout (read by parent process; must precede log output)
    println!("{}", local_addr.port());
    info!("agent started: {}", local_addr);

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("connection accepted: {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                error!("connection handler error: {}", e);
            }
        });
    }
}

/// Start the server with an auto-assigned port.
pub async fn run_auto_port() -> Result<()> {
    run(0).await
}

/// Handle a single connection.
async fn handle_connection(stream: tokio::net::TcpStream) -> Result<()> {
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
            handle_connection(stream).await.unwrap();
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "test".to_string()],
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
            handle_connection(stream).await
        });

        // Act
        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        drop(stream); // disconnect immediately

        // Assert
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}
