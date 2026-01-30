use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use std::path::Path;
use tokio::net::UnixStream;

/// Connect to the agent via Unix socket and execute a command.
pub async fn execute(socket_path: &Path, args: Vec<String>) -> Result<ExecuteResponse> {
    let stream = UnixStream::connect(socket_path).await?;
    let (mut reader, mut writer) = stream.into_split();

    let request = ExecuteRequest { args };
    write_message(&mut writer, &request).await?;
    let response: ExecuteResponse = read_message(&mut reader).await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssh_common::protocol::write_message as server_write;
    use tempfile::TempDir;
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn sends_command_to_agent_and_receives_response() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let sock_path = tmp.path().join("test.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();

        let sock_path_clone = sock_path.clone();
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
        let response = execute(&sock_path_clone, vec!["test".to_string()])
            .await
            .unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(response.stdout, b"mocked output");
    }

    #[tokio::test]
    async fn returns_error_on_connection_failure() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let sock_path = tmp.path().join("nonexistent.sock");

        // Act
        let result = execute(&sock_path, vec!["test".to_string()]).await;

        // Assert
        assert!(result.is_err());
    }
}
