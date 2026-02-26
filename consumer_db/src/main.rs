// An example of an SPN Consumer endpoint crate.
//
// This is a simple PostgreSQL client that connects over SPN
// using tokio_postgres's `connect_raw` method.

use tracing::{error, info};

use ep_lib::core::create_spn_consumer_endpoint;

use tokio_postgres::{Config, NoTls};

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
    let consumer = create_spn_consumer_endpoint(
        "https://spn-hub.example.com:4433",
        "/path/to/cert.pem",
        "/path/to/key.pem",
        "/path/to/ca.pem",
    ).await?;
    info!("SpnConsumer created. Background connection maintenance is running.");

    info!("Postgres Client over QUIC");
    match consumer.open_stream().await {
        Ok(stream) => {
            info!("Successfully opened a QUIC stream.");

            // 2. Connect to PostgreSQL using the `connect_raw` method.
            // https://docs.rs/tokio-postgres/latest/tokio_postgres/config/struct.Config.html#method.connect_raw
            let mut config = Config::new();
            config.user("postgres");
            config.password("Dgtb353cSW");
            config.dbname("postgres");

            info!("Attempting to connect to Postgres with a 10-second timeout...");
            let connect_future = config.connect_raw(stream, NoTls);
            let (client, connection) = match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                connect_future,
            )
            .await
            {
                Ok(Ok(res)) => res, // Connection successful
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => return Err("Connection timed out".into()),
            };

            // The connection object performs the actual communication with the database,
            // so spawn it off to run in the background.
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    error!("connection error: {}", e);
                }
            });

            // 3. Execute a query and retrieve the results.
            info!("Executing query...");
            let rows = client
                .query("SELECT id, name, origin, price FROM fruit", &[])
                .await?;

            info!("--- Fruit List ---");
            for row in &rows {
                let id: &str = row.get("id");
                let name: &str = row.get("name");
                let origin: &str = row.get("origin");
                let price: i32 = row.get("price");
                info!(
                    "id: {}, name: {}, origin: {}, price: {}",
                    id, name, origin, price
                );
            }
            info!("--------------------");
        }
        Err(e) => {
            error!("Failed to open a QUIC stream: {}", e);
        }
    }

    info!("Application shutting down. The Consumer will be dropped, stopping background tasks.");
    Ok(())
}