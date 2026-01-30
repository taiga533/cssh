use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use tokio::net::{TcpListener, TcpStream};

/// Mock handler equivalent to the agent (for testing).
async fn mock_agent_handler(stream: TcpStream) {
    let (mut reader, mut writer) = stream.into_split();
    loop {
        let request: ExecuteRequest = match read_message(&mut reader).await {
            Ok(req) => req,
            Err(_) => return,
        };
        let output = std::process::Command::new(&request.args[0])
            .args(&request.args[1..])
            .output()
            .unwrap();
        let response = ExecuteResponse {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
        };
        write_message(&mut writer, &response).await.unwrap();
    }
}

#[cfg(target_family = "unix")]
mod unix_tests {
    use super::*;

    #[tokio::test]
    async fn command_execution_between_agent_and_remote_works_correctly() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            mock_agent_handler(stream).await;
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // Act
        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "integration test".to_string()],
        };
        write_message(&mut writer, &request).await.unwrap();
        let response: ExecuteResponse = read_message(&mut reader).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(
            String::from_utf8_lossy(&response.stdout).trim(),
            "integration test"
        );
    }

    #[tokio::test]
    async fn multiple_commands_can_be_executed_sequentially() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            mock_agent_handler(stream).await;
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // Act & Assert - first
        let req1 = ExecuteRequest {
            args: vec!["echo".to_string(), "first".to_string()],
        };
        write_message(&mut writer, &req1).await.unwrap();
        let resp1: ExecuteResponse = read_message(&mut reader).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&resp1.stdout).trim(), "first");

        // Act & Assert - second
        let req2 = ExecuteRequest {
            args: vec!["echo".to_string(), "second".to_string()],
        };
        write_message(&mut writer, &req2).await.unwrap();
        let resp2: ExecuteResponse = read_message(&mut reader).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&resp2.stdout).trim(), "second");
    }
}

#[cfg(target_family = "windows")]
mod windows_tests {
    use super::*;

    #[tokio::test]
    async fn command_execution_between_agent_and_remote_works_correctly() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            mock_agent_handler(stream).await;
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // Act
        let request = ExecuteRequest {
            args: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo".to_string(),
                "integration test".to_string(),
            ],
        };
        write_message(&mut writer, &request).await.unwrap();
        let response: ExecuteResponse = read_message(&mut reader).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(
            String::from_utf8_lossy(&response.stdout).trim(),
            "integration test"
        );
    }

    #[tokio::test]
    async fn multiple_commands_can_be_executed_sequentially() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            mock_agent_handler(stream).await;
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // Act & Assert - first
        let req1 = ExecuteRequest {
            args: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo".to_string(),
                "first".to_string(),
            ],
        };
        write_message(&mut writer, &req1).await.unwrap();
        let resp1: ExecuteResponse = read_message(&mut reader).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&resp1.stdout).trim(), "first");

        // Act & Assert - second
        let req2 = ExecuteRequest {
            args: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo".to_string(),
                "second".to_string(),
            ],
        };
        write_message(&mut writer, &req2).await.unwrap();
        let resp2: ExecuteResponse = read_message(&mut reader).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&resp2.stdout).trim(), "second");
    }
}
