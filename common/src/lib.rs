use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// The wire protocol enum. Every message between client and server is one of
/// these variants, serialized with bincode and wrapped in a length-prefixed frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Client -> Server login request.
    LoginRequest { username: String, password: String },

    /// Server -> Client login success response.
    LoginResponse { success: bool, message: String },

    /// Client -> Server message.
    /// The server knows who sent it auth state, so the client doesn't need to include username.
    ChatMessage { content: String },

    /// Server -> Client broadcasting a client message to all connected clients.
    ChatBroadcast {
        username: String,
        content: String,
        timestamp: String,
    },

    /// Server -> Client  system/admin announcements eg: join/leave notices, errors, etc.
    ServerNotice { message: String },

    /// Client -> Server request to get message history.
    HistoryRequest { count: u32 },

    /// Server -> Client response with message history.
    HistoryResponse { messages: Vec<ChatEntry> },
}

/// A single chat message entry, used in history responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatEntry {
    pub username: String,
    pub content: String,
    pub timestamp: String,
}

/// The maximum frame size we accept is 1mb to prevents a malicious or buggy
/// client from sending an oversized length prefix and causing an out-of-memory crash.
const MAX_FRAME_SIZE: usize = 1_048_576;

/// Serialize a Message and write it to the stream as a length-prefixed frame.
/// Wire format: [4-byte big-endian length][bincode payload]
pub async fn write_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    msg: &Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Serialize our Message enum into a Vec<u8>.
    let payload: Vec<u8> = bincode::serialize(msg)?;

    // Cast payload length to u32 and convert to big-endian (network byte order).
    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;

    // Write the actual serialized message bytes.
    writer.write_all(&payload).await?;

    // Send bytes over the network.
    writer.flush().await?;

    Ok(())
}

/// Read a length-prefixed frame from the stream and deserialize it.
/// Returns Ok(Some(msg)) on success, Ok(None) on clean disconnect (EOF),
/// or Err on protocol/IO errors.
pub async fn read_frame<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<Option<Message>, Box<dyn std::error::Error + Send + Sync>> {
    // Read the 4-byte length prefix.
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {} // Got our 4 bytes, continue.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            // The peer closed the connection cleanly without error.
            return Ok(None);
        }
        Err(e) => return Err(e.into()), // I/O error
    }

    // Parse the length and validate it.
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > MAX_FRAME_SIZE {
        return Err(format!("Frame too large: {len} bytes (max {MAX_FRAME_SIZE})").into());
    }

    // Allocate a buffer and read the payload.
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;

    // Deserialize the payload back into a Message enum.
    let msg: Message = bincode::deserialize(&payload)?;

    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_frame_round_trip() {
        // Create an in-memory bidirectional stream.
        let (mut client, mut server) = tokio::io::duplex(1024);

        // Create a test message.
        let original = Message::ChatBroadcast {
            username: "alice".to_string(),
            content: "hello world".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };

        // Write the frame to one side of the duplex.
        write_frame(&mut client, &original).await.unwrap();

        // Read the frame from the other side and verify it round-trips correctly.
        let received = read_frame(&mut server)
            .await
            .unwrap() // Unwrap Result
            .unwrap(); // Unwrap Option

        assert_eq!(format!("{:?}", original), format!("{:?}", received));
    }

    #[tokio::test]
    async fn test_eof_returns_none() {
        // Create a duplex and immediately drop the write side.
        // This simulates a peer that connects and then disconnects.
        let (client, mut server) = tokio::io::duplex(1024);
        drop(client);

        let result = read_frame(&mut server).await.unwrap();
        assert!(result.is_none(), "Expected None on EOF");
    }

    #[tokio::test]
    async fn test_all_message_variants() {
        // Verify every variant of Message can round-trip through framing.
        let messages = vec![
            Message::LoginRequest {
                username: "bob".to_string(),
                password: "secret".to_string(),
            },
            Message::LoginResponse {
                success: true,
                message: "Welcome!".to_string(),
            },
            Message::ChatMessage {
                content: "test message".to_string(),
            },
            Message::ChatBroadcast {
                username: "bob".to_string(),
                content: "test message".to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
            },
            Message::ServerNotice {
                message: "Bob joined.".to_string(),
            },
            Message::HistoryRequest { count: 50 },
            Message::HistoryResponse {
                messages: vec![ChatEntry {
                    username: "alice".to_string(),
                    content: "earlier message".to_string(),
                    timestamp: "2025-12-31T23:59:00Z".to_string(),
                }],
            },
        ];

        for original in &messages {
            let (mut client, mut server) = tokio::io::duplex(4096);

            write_frame(&mut client, original).await.unwrap();

            let received = read_frame(&mut server).await.unwrap().unwrap();

            assert_eq!(
                format!("{:?}", original),
                format!("{:?}", received),
                "Round-trip failed for variant: {:?}",
                original
            );
        }
    }
}
