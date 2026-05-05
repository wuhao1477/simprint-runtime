use super::error::{Result, RuntimeError};
use super::events::{RuntimeEventEnvelope, event_channel};
use super::host::{DispatchControl, RuntimeHost};
use crate::infrastructure::diagnostics::{log_debug, log_error, log_info, log_warn};
use crate::infrastructure::ipc::{
    ErrorCode, Message, MessageTransport, MessageType, Topic, stdio_transport,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

pub struct RuntimeBootstrap<R, W> {
    host: std::sync::Arc<RuntimeHost>,
    transport: MessageTransport<R, W>,
    event_rx: mpsc::UnboundedReceiver<RuntimeEventEnvelope>,
    handshake_complete: bool,
}

impl RuntimeBootstrap<tokio::io::Stdin, tokio::io::Stdout> {
    pub fn stdio() -> Result<Self> {
        let (events, event_rx) = event_channel();
        Ok(Self {
            host: RuntimeHost::default(events),
            transport: stdio_transport(),
            event_rx,
            handshake_complete: false,
        })
    }
}

impl<R, W> RuntimeBootstrap<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub async fn run(mut self) -> Result<()> {
        self.host.start().await?;

        loop {
            tokio::select! {
                maybe_event = self.event_rx.recv() => {
                    match maybe_event {
                        Some(event) => self.forward_event(event).await?,
                        None => log_warn("bootstrap", "event channel closed"),
                    }
                }
                incoming = self.transport.recv() => {
                    match incoming {
                        Ok(message) => {
                            if let Some(control) = self.process_incoming(message).await? {
                                if matches!(control, DispatchControl::Shutdown) {
                                    break;
                                }
                            }
                        }
                        Err(crate::infrastructure::ipc::IpcError::ConnectionClosed) => {
                            self.host.shutdown_due_to_disconnect().await?;
                            break;
                        }
                        Err(error) => {
                            return Err(RuntimeError::from(error));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_incoming(&mut self, message: Message) -> Result<Option<DispatchControl>> {
        if message.msg_type != MessageType::Request {
            log_warn(
                "bootstrap",
                format!("dropping non-request inbound frame: {:?}", message.msg_type),
            );
            return Ok(None);
        }

        if !self.handshake_complete && message.topic != Topic::Handshake {
            let response = Message::error_response_payload(
                message.msg_id,
                message.topic,
                ErrorCode::HandshakeRequired,
                &super::api::ErrorResponse {
                    message: "handshake is required before other commands".into(),
                },
            )?;
            self.transport.send(&response).await?;
            return Ok(Some(DispatchControl::Continue));
        }

        let result = self.host.handle_request(message.clone()).await;
        match result {
            Ok(dispatch) => {
                if message.topic == Topic::Handshake {
                    self.handshake_complete = true;
                    log_info("bootstrap", "peer handshake completed");
                }
                self.transport.send(&dispatch.response).await?;
                Ok(Some(dispatch.control))
            }
            Err(error) => {
                log_error("bootstrap", format!("request handling failed: {}", error));
                let response = self.host.error_response_for(&message, &error)?;
                self.transport.send(&response).await?;
                Ok(Some(DispatchControl::Continue))
            }
        }
    }

    async fn forward_event(&mut self, event: RuntimeEventEnvelope) -> Result<()> {
        let message = Message::event_payload(Topic::RuntimeEvent, &event)?;
        self.transport.send(&message).await?;
        log_debug(
            "bootstrap",
            format!("forwarded runtime event '{}'", event.name),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        DestroyContextRequest, EmptyPayload, HandshakeRequest, InitializeContextRequest,
        RuntimeContextInput,
    };
    use crate::infrastructure::ipc::{Message, MessageTransport, MessageType, PROTOCOL_VERSION};
    use tokio::io::duplex;

    async fn recv_response<R, W>(transport: &mut MessageTransport<R, W>, msg_id: u32) -> Message
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        loop {
            let message = transport.recv().await.unwrap();
            if message.msg_type == MessageType::Response && message.msg_id == msg_id {
                return message;
            }
        }
    }

    #[tokio::test]
    async fn bootstrap_requires_handshake_before_initialize() {
        let (client, server) = duplex(32 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client);
        let (server_reader, server_writer) = tokio::io::split(server);

        let (events, event_rx) = event_channel();
        let host = RuntimeHost::default(events);
        let bootstrap = RuntimeBootstrap {
            host,
            transport: MessageTransport::new(server_reader, server_writer),
            event_rx,
            handshake_complete: false,
        };

        let server_task = tokio::spawn(async move {
            bootstrap.run().await.unwrap();
        });

        let mut client_transport = MessageTransport::new(client_reader, client_writer);
        let init = Message::request_payload(
            Topic::InitializeContext,
            &InitializeContextRequest {
                context: RuntimeContextInput::default(),
            },
        )
        .unwrap();
        let init_id = init.msg_id;
        client_transport.send(&init).await.unwrap();
        let response = recv_response(&mut client_transport, init_id).await;
        assert_eq!(response.error_code, ErrorCode::HandshakeRequired.as_i32());

        let handshake = Message::request_payload(
            Topic::Handshake,
            &HandshakeRequest {
                protocol_version: PROTOCOL_VERSION,
                client_name: "simprint".into(),
                client_version: "0.1.0".into(),
            },
        )
        .unwrap();
        let handshake_id = handshake.msg_id;
        client_transport.send(&handshake).await.unwrap();
        let response = recv_response(&mut client_transport, handshake_id).await;
        assert_eq!(response.error_code, 0);

        let init = Message::request_payload(
            Topic::InitializeContext,
            &InitializeContextRequest {
                context: RuntimeContextInput::default(),
            },
        )
        .unwrap();
        let init_id = init.msg_id;
        client_transport.send(&init).await.unwrap();
        let response = recv_response(&mut client_transport, init_id).await;
        assert_eq!(response.error_code, 0);

        let destroy =
            Message::request_payload(Topic::DestroyContext, &DestroyContextRequest::default())
                .unwrap();
        let destroy_id = destroy.msg_id;
        client_transport.send(&destroy).await.unwrap();
        let response = recv_response(&mut client_transport, destroy_id).await;
        assert_eq!(response.error_code, 0);

        let shutdown = Message::request_payload(Topic::Shutdown, &EmptyPayload::default()).unwrap();
        let shutdown_id = shutdown.msg_id;
        client_transport.send(&shutdown).await.unwrap();
        let response = recv_response(&mut client_transport, shutdown_id).await;
        assert_eq!(response.error_code, 0);

        server_task.await.unwrap();
    }
}
