use cssh_common::error::Result;
use cssh_common::protocol::{read_message, write_message, ExecuteRequest, ExecuteResponse};
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::executor;

/// エージェントサーバーを指定ポートで起動する
pub async fn run(port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let local_addr = listener.local_addr()?;
    // ポート番号を標準出力に出力（親プロセスが読み取る。必ずログより先に出力する）
    println!("{}", local_addr.port());
    info!("エージェント起動: {}", local_addr);

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("接続受付: {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                error!("接続処理エラー: {}", e);
            }
        });
    }
}

/// 自動割り当てポートでサーバーを起動する
pub async fn run_auto_port() -> Result<()> {
    run(0).await
}

/// 1つの接続を処理する
async fn handle_connection(stream: tokio::net::TcpStream) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    loop {
        let request: ExecuteRequest = match read_message(&mut reader).await {
            Ok(req) => req,
            Err(cssh_common::error::CsshError::Io(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                info!("クライアント切断");
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        info!("コマンド実行: {:?}", request.args);
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
    async fn サーバーがコマンドを実行してレスポンスを返す() {
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
    async fn クライアント切断時にサーバーが正常終了する() {
        // Arrange
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream).await
        });

        // Act
        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        drop(stream); // 即切断

        // Assert
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}
