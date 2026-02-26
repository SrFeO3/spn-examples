// An example of an SPN Provider endpoint crate.
//
// This is a simple HTTP/1.0 server provided over SPN.

use ep_lib::core::create_spn_provider_endpoint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .with_current_span(false)
        .init();

    info!("My new application started!");

    let provider = create_spn_provider_endpoint(
        "https://spn-hub.example.com:4433",
        "/path/to/cert.pem",
        "/path/to/key.pem",
        "/path/to/ca.pem",
    ).await?;
    info!("SpnProvider created. Background connection maintenance is running.");

    info!("Simple HTTP 1.0 Server");
    loop {
        tokio::select! {
            // Wait for a shutdown signal (Ctrl-C).
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl-C received, shutting down.");
                // ここで、graceful リタイアのメソッドを呼び出してもらう？ provider.setGracefulRetire();
                break;
            }

            // Accept a new stream.
            result = provider.accept_stream() => {
                match result {
                    Ok(mut stream) => {
                        info!("New stream accepted! Spawning a handler task.");
                        // Spawn a new task to process this stream asynchronously.
                        tokio::spawn(async move {
                            // Wait for and read the request from the client.
                            let mut request_buffer = vec![0; 4096];
                            if let Err(e) = stream.read(&mut request_buffer).await {
                                error!("Failed to read from stream: {}", e);
                                return;
                            }
                            info!("Received request, sending response...");

                            // Prepare a simple HTTP/1.0 response.
                            let body = "<html><body><h1>Hello from SPN!</h1></body></html>";
                            let response = format!(
                                "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );

                            // Send the response and gracefully close the write-side of the stream.
                            if let Err(e) = stream.write_all(response.as_bytes()).await {
                                error!("Failed to send response: {}", e);
                            } else if let Err(e) = stream.shutdown().await {
                                error!("Failed to shutdown stream: {}", e);
                            } else {
                                info!("Response sent and stream finished.");
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept a QUIC stream: {}", e);
                        // Depending on the error, we might want to break the loop, but here we continue.
                    }
                }
            }
        }
    }

    info!("Application shutting down. Provider will be dropped, stopping background tasks.");
    Ok(())
}
