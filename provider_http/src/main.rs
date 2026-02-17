// An example of an SPN Provider endpoint crate.
//
// This is a simple HTTP/1.0 server provided over SPN.

use ep_lib::client_core::create_spn_endpoint;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .with_current_span(false)
        .init();

    info!("My new application started!");

    let provider = create_spn_endpoint(
        &"chipin://testing123.wgd.example.com:4433",
        &"../certc2/client1.pem",
        &"../certc2/client1-key.pem",
        &"../certs2/ca.pem",
        &[b"sc01-provider"], // for development
    )
    .await?;
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
                    Ok((mut send_stream, mut recv_stream)) => {
                        info!("New stream accepted! Spawning a handler task.");
                        // Spawn a new task to process this stream asynchronously.
                        tokio::spawn(async move {
                            // Wait for and read the request from the client.
                            let mut request_buffer = vec![0; 4096];
                            if let Err(e) = recv_stream.read(&mut request_buffer).await {
                                error!("Failed to read from stream: {}", e);
                                return;
                            }
                            info!("Received request, sending response...");

                            // Prepare a simple HTTP/1.0 response.
                            let body = "<html><body><h1>Hello, World!</h1></body></html>";
                            let response = format!(
                                "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );

                            // Send the response and close the stream.
                            if let Err(e) = send_stream.write_all(response.as_bytes()).await {
                                error!("Failed to send response: {}", e);
                            // `and_then` cannot be used because the error types are different. Call `finish()` separately.
                            } else if let Err(e) = send_stream.finish() {
                                error!("Failed to finish stream: {}", e);
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
