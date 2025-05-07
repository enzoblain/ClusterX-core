use crate::handler::candle::proceed_data;
use crate::CONFIG;
use crate::providers;

use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::{TcpStream, TcpListener};
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message, WebSocketStream};

pub static CLIENTS: Lazy<Arc<Mutex<Vec<Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>>>>> = Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

pub async fn connect_to_provider_websocket() {
    // Load the configuration
    // To create the WebSocket connection
    let config = CONFIG.get().unwrap().lock().await.clone();
    let url = providers::general::build_stream_url(&config.stream.provider, &config.params.symbols, &config.stream.stream_type);

    // Connect to the WebSocket stream
    let (provider_ws_stream, _) = connect_async(&url)
        .await
        .expect("Failed to connect to Binance");

    let (mut provider_write, mut provider_read) = provider_ws_stream.split();

    // Wait for messages from the WebSocket stream
    // and get the candle data
    // Or handle ping/pong messages
    while let Some(message) = provider_read.next().await {
        match message {
            Ok(Message::Text(text)) => {
                let candle = providers::general::parse_candle(&text, &config.stream.provider).unwrap();

                proceed_data(candle).await;
            },
            Ok(Message::Ping(ping)) => {
                // Handle ping messages
                provider_write.send(Message::Pong(ping)).await.unwrap();
            },
            Ok(_) => (),
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}

pub async fn connect_to_intra_websocket() {
    // Load the url
    let url = common::WEBSOCKET_URL.as_str();

    // Set up a TCP listener
    let listener = TcpListener::bind(url)
        .await
        .expect("Unable to bind TCP listener");

    // Start the WebSocket server
    // and accept incoming WebSocket connections
    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream)
            .await
            .expect("Error during WebSocket handshake");

        let (write, mut read) = ws_stream.split();

        // Add the new client to the list of clients
        // Use an Arc and Mutex to share the client between tasks
        // and ensure thread safety
        let client = Arc::new(Mutex::new(write));
        add_client(client.clone()).await;

        // Handle incoming messages from the WebSocket client
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(_)) => {
                   let mut write = client.lock().await;
                    write.send(Message::Text("You can't send messages to this server".into()))
                        .await
                        .expect("Error sending message");

                    break;
                }
                Ok(Message::Close(_)) => {
                    // Handle the close message
                    break;
                }
                Err(e) => {
                    println!("Error: {}", e);
                    break;
                }
                _ => {
                    let mut write = client.lock().await;
                    write.send(Message::Text("You can't send messages to this server".into()))
                        .await
                        .expect("Error sending message");

                    break;
                }
            }
        }

        // Remove the client from the list of clients
        remove_client(client.clone()).await;
    }
}

// Add a new client to the list of clients
// Separte function to avoid long locks
pub async fn add_client(client: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>) {
    let mut clients = CLIENTS.lock().await;
    clients.push(client);
}

// Same thing as above for removing a client
pub async fn remove_client(client: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>) {
    let mut clients = CLIENTS.lock().await;
    clients.retain(|c| !Arc::ptr_eq(c, &client));
}

// Send a message to all connected clients
pub async fn send_message_to_clients(message: &String) {
    let clients = CLIENTS.lock().await;

    for client in clients.iter() {
        let mut write = client.lock().await;
        write.send(Message::Text(message.into())).await.expect("Error sending message");
    }
}