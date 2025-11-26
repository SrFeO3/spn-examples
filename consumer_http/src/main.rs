// An example of an SPN Consumer endpoint crate.
//
// This is a simple HTTP/1.1 client that connects over SPN
// using Hyper.
//

use tracing::{error, info};

use quinn::RecvStream;
use quinn::SendStream;

use ep_lib::client_core::create_spn_endpoint;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::ReadBuf;

use tokio::io;

use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::Request;
use hyper_util::rt::TokioIo;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging. The library user is expected to do this.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .with_current_span(false)
        .init();

    info!("My new application started!");

    // 1. Call the library's main function to get the Consumer object.
    //    This establishes and maintains the QUIC connection in the background.
    let consumer = create_spn_endpoint(
        &"chipin://spnhub.wgd.example.com:4433",
        &"../cert_client/client2.pem",
        &"../cert_client/client2-key.pem",
        &"../cert_server/ca.pem",
        &[b"sc01-consumer"], // for development
    )
    .await?;
    info!("SpnConsumer created. Background connection maintenance is running.");

    info!("H1 Client over QUIC");
    match consumer.open_stream().await {
        Ok((send_stream, recv_stream)) => {
            info!("Successfully opened a QUIC stream.");

            // 2. Combine the send and receive streams into a single bidirectional stream.
            let adapted_stream = TokioStreamAdapter::new(send_stream, recv_stream);

            // 3. Wrap the stream in TokioIo to make it compatible with Hyper.
            let io = TokioIo::new(adapted_stream);

            let (mut sender, connection) = hyper::client::conn::http1::handshake(io).await?;

            // The connection object performs the actual communication with the server,
            // so spawn it off to run in the background.
            tokio::spawn(async move {
                if let Err(err) = connection.await {
                    error!("Connection failed: {:?}", err);
                }
            });

            // 4. Send an HTTP request.
            let request = Request::builder()
                .method("GET")
                .uri("/")
                .header("Host", "example.com")
                .body(Empty::<Bytes>::new())?;

            match sender.send_request(request).await {
                Ok(response) => {
                    println!("Response status: {}", response.status());
                    // Read the response body.
                    let body_bytes = response.into_body().collect().await?.to_bytes();
                    println!("Response body: {:?}", String::from_utf8_lossy(&body_bytes));
                }
                Err(e) => error!("Failed to send request: {:?}", e),
            }
        }
        Err(e) => {
            error!("Failed to open a QUIC stream: {}", e);
        }
    }

    info!("Application shutting down. The Consumer will be dropped, stopping background tasks.");
    Ok(())
}

pub struct TokioStreamAdapter {
    send: SendStream,
    recv: RecvStream,
}

impl TokioStreamAdapter {
    pub fn new(send: SendStream, recv: RecvStream) -> Self {
        Self { send, recv }
    }
}

impl AsyncRead for TokioStreamAdapter {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().recv)
            .poll_read(cx, buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl AsyncWrite for TokioStreamAdapter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().send)
            .poll_write(cx, buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().send)
            .poll_flush(cx)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().send)
            .poll_shutdown(cx)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}
