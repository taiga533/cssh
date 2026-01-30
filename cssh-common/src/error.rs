/// cssh共通エラー型
#[derive(Debug, thiserror::Error)]
pub enum CsshError {
    #[error("IOエラー: {0}")]
    Io(#[from] std::io::Error),

    #[error("シリアライズエラー: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("不正なメッセージ長: {0}")]
    InvalidMessageLength(u32),

    #[error("接続エラー: {0}")]
    Connection(String),

    #[error("コマンド実行エラー: {0}")]
    Execution(String),
}

pub type Result<T> = std::result::Result<T, CsshError>;
