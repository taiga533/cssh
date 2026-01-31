use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::ConnectionInfo;

/// Connect to the agent and execute a command with authentication.
///
/// Dispatches to Unix socket or TCP based on the connection info.
pub async fn execute(conn: ConnectionInfo, args: Vec<String>) -> Result<ExecuteResponse> {
    match conn {
        ConnectionInfo::UnixSocket { path, token } => {
            #[cfg(unix)]
            {
                let stream = tokio::net::UnixStream::connect(&path).await?;
                let (reader, writer) = stream.into_split();
                execute_on_stream(reader, writer, token, args).await
            }
            #[cfg(not(unix))]
            {
                let _ = (path, token, args);
                Err(cssh_common::error::CsshError::Execution(
                    "Unix socket not supported on this platform".to_string(),
                ))
            }
        }
        ConnectionInfo::Tcp { port, token } => {
            let stream = TcpStream::connect(("127.0.0.1", port)).await?;
            let (reader, writer) = stream.into_split();
            execute_on_stream(reader, writer, token, args).await
        }
    }
}

/// Send a request and receive a response over an async stream.
async fn execute_on_stream<R, W>(
    mut reader: R,
    mut writer: W,
    token: String,
    args: Vec<String>,
) -> Result<ExecuteResponse>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let request = ExecuteRequest { args, token };
    write_message(&mut writer, &request).await?;
    let response: ExecuteResponse = read_message(&mut reader).await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssh_common::protocol::write_message as server_write;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn sends_command_to_agent_and_receives_response_via_tcp() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut reader, mut writer) = stream.into_split();
            let _req: ExecuteRequest = read_message(&mut reader).await.unwrap();
            let resp = ExecuteResponse {
                exit_code: 0,
                stdout: b"mocked output".to_vec(),
                stderr: Vec::new(),
            };
            server_write(&mut writer, &resp).await.unwrap();
        });

        let conn = ConnectionInfo::Tcp {
            port,
            token: String::new(),
        };

        // Act
        let response = execute(conn, vec!["test".to_string()]).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(response.stdout, b"mocked output");
    }

    #[tokio::test]
    async fn sends_token_in_request_via_tcp() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut reader, mut writer) = stream.into_split();
            let req: ExecuteRequest = read_message(&mut reader).await.unwrap();
            let resp = ExecuteResponse {
                exit_code: 0,
                stdout: Vec::new(),
                stderr: Vec::new(),
            };
            server_write(&mut writer, &resp).await.unwrap();
            req.token
        });

        let conn = ConnectionInfo::Tcp {
            port,
            token: "my_secret_token".to_string(),
        };

        // Act
        execute(conn, vec!["test".to_string()]).await.unwrap();

        // Assert
        let received_token = handle.await.unwrap();
        assert_eq!(received_token, "my_secret_token");
    }

    #[tokio::test]
    async fn returns_error_on_tcp_connection_failure() {
        // Arrange
        let conn = ConnectionInfo::Tcp {
            port: 19999,
            token: String::new(),
        };

        // Act
        let result = execute(conn, vec!["test".to_string()]).await;

        // Assert
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sends_command_to_agent_and_receives_response_via_unix_socket() {
        // Arrange
        let tmp_dir = tempfile::tempdir().unwrap();
        let sock_path = tmp_dir.path().join("test.sock");
        let sock_path_str = sock_path.to_string_lossy().to_string();

        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut reader, mut writer) = stream.into_split();
            let _req: ExecuteRequest = read_message(&mut reader).await.unwrap();
            let resp = ExecuteResponse {
                exit_code: 42,
                stdout: b"unix output".to_vec(),
                stderr: Vec::new(),
            };
            server_write(&mut writer, &resp).await.unwrap();
        });

        let conn = ConnectionInfo::UnixSocket {
            path: sock_path_str,
            token: "uds_token".to_string(),
        };

        // Act
        let response = execute(conn, vec!["hello".to_string()]).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 42);
        assert_eq!(response.stdout, b"unix output");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn returns_error_on_unix_socket_connection_failure() {
        // Arrange
        let conn = ConnectionInfo::UnixSocket {
            path: "/tmp/nonexistent-cssh-test.sock".to_string(),
            token: String::new(),
        };

        // Act
        let result = execute(conn, vec!["test".to_string()]).await;

        // Assert
        assert!(result.is_err());
    }
}
