use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{CsshError, Result};

/// Maximum message size (16MB).
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Command execution request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecuteRequest {
    /// Command and arguments to execute.
    pub args: Vec<String>,
}

/// Command execution response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecuteResponse {
    /// Exit code.
    pub exit_code: i32,
    /// Standard output.
    pub stdout: Vec<u8>,
    /// Standard error output.
    pub stderr: Vec<u8>,
}

/// Serialize a message with a length prefix and return as bytes.
pub fn encode_message<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(msg)?;
    let len = json.len() as u32;
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&json);
    Ok(buf)
}

/// Deserialize a message from length-prefixed bytes.
pub fn decode_message<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<(T, usize)> {
    if data.len() < 4 {
        return Err(CsshError::Connection("data too short".to_string()));
    }
    let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if len > MAX_MESSAGE_SIZE {
        return Err(CsshError::InvalidMessageLength(len));
    }
    let total = 4 + len as usize;
    if data.len() < total {
        return Err(CsshError::Connection("incomplete data".to_string()));
    }
    let msg: T = serde_json::from_slice(&data[4..total])?;
    Ok((msg, total))
}

/// Read a message from an async stream.
pub async fn read_message<T: for<'de> Deserialize<'de>, R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<T> {
    let len = reader.read_u32().await?;
    if len > MAX_MESSAGE_SIZE {
        return Err(CsshError::InvalidMessageLength(len));
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await?;
    let msg: T = serde_json::from_slice(&buf)?;
    Ok(msg)
}

/// Write a message to an async stream.
pub async fn write_message<T: Serialize, W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    msg: &T,
) -> Result<()> {
    let json = serde_json::to_vec(msg)?;
    let len = json.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&json).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serialization_and_deserialization_works_correctly() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["ls".to_string(), "-la".to_string()],
        };

        // Act
        let encoded = encode_message(&request).unwrap();
        let (decoded, consumed): (ExecuteRequest, usize) = decode_message(&encoded).unwrap();

        // Assert
        assert_eq!(decoded, request);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn response_serialization_and_deserialization_works_correctly() {
        // Arrange
        let response = ExecuteResponse {
            exit_code: 0,
            stdout: b"hello\n".to_vec(),
            stderr: Vec::new(),
        };

        // Act
        let encoded = encode_message(&response).unwrap();
        let (decoded, _): (ExecuteResponse, usize) = decode_message(&encoded).unwrap();

        // Assert
        assert_eq!(decoded, response);
    }

    #[test]
    fn returns_error_when_data_is_too_short() {
        // Arrange
        let data = [0u8; 2];

        // Act
        let result = decode_message::<ExecuteRequest>(&data);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_when_message_length_exceeds_limit() {
        // Arrange
        let len = (MAX_MESSAGE_SIZE + 1).to_be_bytes();
        let data = [len[0], len[1], len[2], len[3]];

        // Act
        let result = decode_message::<ExecuteRequest>(&data);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_when_data_is_incomplete() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["test".to_string()],
        };
        let encoded = encode_message(&request).unwrap();
        let incomplete = &encoded[..encoded.len() - 2];

        // Act
        let result = decode_message::<ExecuteRequest>(incomplete);

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn async_stream_read_and_write_works_correctly() {
        // Arrange
        let request = ExecuteRequest {
            args: vec!["echo".to_string(), "hello".to_string()],
        };
        let mut buf = Vec::new();

        // Act
        write_message(&mut buf, &request).await.unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let decoded: ExecuteRequest = read_message(&mut cursor).await.unwrap();

        // Assert
        assert_eq!(decoded, request);
    }
}
