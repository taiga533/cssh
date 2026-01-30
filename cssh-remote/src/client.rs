use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use tokio::net::TcpStream;

/// Connect to the agent via TCP and execute a command with authentication.
pub async fn execute(port: u16, token: String, args: Vec<String>) -> Result<ExecuteResponse> {
    let stream = TcpStream::connect(("127.0.0.1", port)).await?;
    let (mut reader, mut writer) = stream.into_split();

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
    async fn sends_command_to_agent_and_receives_response() {
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

        // Act
        let response = execute(port, String::new(), vec!["test".to_string()]).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(response.stdout, b"mocked output");
    }

    #[tokio::test]
    async fn sends_token_in_request() {
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

        // Act
        execute(port, "my_secret_token".to_string(), vec!["test".to_string()])
            .await
            .unwrap();

        // Assert
        let received_token = handle.await.unwrap();
        assert_eq!(received_token, "my_secret_token");
    }

    #[tokio::test]
    async fn returns_error_on_connection_failure() {
        // Arrange - connect to a non-existent port
        // Act
        let result = execute(19999, String::new(), vec!["test".to_string()]).await;

        // Assert
        assert!(result.is_err());
    }
}
