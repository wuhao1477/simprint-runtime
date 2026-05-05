use super::error::{IpcError, Result};
use super::message::Message;
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const READ_BUFFER_CAPACITY: usize = 64 * 1024;

pub struct MessageTransport<R, W> {
    reader: R,
    writer: W,
    read_buffer: BytesMut,
}

impl<R, W> MessageTransport<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            read_buffer: BytesMut::with_capacity(READ_BUFFER_CAPACITY),
        }
    }

    pub async fn send(&mut self, message: &Message) -> Result<()> {
        let encoded = message.encode()?;
        self.writer
            .write_all(&encoded)
            .await
            .map_err(|error| IpcError::SendFailed(error.to_string()))?;
        self.writer
            .flush()
            .await
            .map_err(|error| IpcError::SendFailed(error.to_string()))?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Message> {
        loop {
            if let Some((message, consumed)) = Message::try_decode(&self.read_buffer)? {
                let _ = self.read_buffer.split_to(consumed);
                return Ok(message);
            }

            let bytes_read = self
                .reader
                .read_buf(&mut self.read_buffer)
                .await
                .map_err(|error| IpcError::ReceiveFailed(error.to_string()))?;

            if bytes_read == 0 {
                return Err(IpcError::ConnectionClosed);
            }
        }
    }
}

pub type StdioTransport = MessageTransport<tokio::io::Stdin, tokio::io::Stdout>;

pub fn stdio_transport() -> StdioTransport {
    MessageTransport::new(tokio::io::stdin(), tokio::io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::ipc::Topic;
    use serde::{Deserialize, Serialize};
    use tokio::io::duplex;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct Payload {
        value: String,
    }

    #[tokio::test]
    async fn transport_roundtrip_over_duplex() {
        let (client, server) = duplex(16 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client);
        let (server_reader, server_writer) = tokio::io::split(server);

        let mut sender = MessageTransport::new(client_reader, client_writer);
        let mut receiver = MessageTransport::new(server_reader, server_writer);

        let message =
            Message::request_payload(Topic::Handshake, &Payload { value: "ok".into() }).unwrap();

        sender.send(&message).await.unwrap();
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.topic, Topic::Handshake);
        assert_eq!(
            received.payload::<Payload>().unwrap(),
            Payload { value: "ok".into() }
        );
    }
}
