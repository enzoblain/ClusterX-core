use common::server::connect_db;
use core::CONFIG;
use core::utils::config;
use core::server::{database, websocket};


#[tokio::main]
async fn main() {
    // Load the configuration
    config::load_config();
    
    let config = CONFIG.get().unwrap().lock().await.clone();

    // Connect to the database
    connect_db().await;
    database::load_last_candles(config.params.symbols.clone()).await;

    // Run our webscocket (to send the data to the users)
    let intra_websocket = tokio::spawn(async move{
        websocket::connect_to_intra_websocket().await;
    });
    
    // Connect to the provider websocket
    let get_data = tokio::spawn(async move {
        websocket::connect_to_provider_websocket().await;
    });

    // Run the two tasks concurrently
    let _ = tokio::join!(intra_websocket, get_data);
}