use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use tokio::net::TcpStream;

/// エージェントに接続してコマンドを実行し、レスポンスを返す
pub async fn execute(port: u16, args: Vec<String>) -> Result<ExecuteResponse> {
    let stream = TcpStream::connect(("127.0.0.1", port)).await?;
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
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn エージェントにコマンドを送信してレスポンスを受け取る() {
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
        let response = execute(port, vec!["test".to_string()]).await.unwrap();

        // Assert
        assert_eq!(response.exit_code, 0);
        assert_eq!(response.stdout, b"mocked output");
    }

    #[tokio::test]
    async fn 接続失敗時にエラーを返す() {
        // Arrange - 存在しないポートに接続
        // Act
        let result = execute(19999, vec!["test".to_string()]).await;

        // Assert
        assert!(result.is_err());
    }
}
