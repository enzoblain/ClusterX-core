use common::Candle;
use tokio::sync::Mutex;
use crate::server::{database::add_candle, websocket::send_message_to_clients};

use once_cell::sync::Lazy;
use serde_json::{Map, Value};
use std::collections::HashMap;

// Store all the candles of each symbol in a hashmap
// The key is the symbol and the value is the actual
pub static CANDLES: Lazy<Mutex<HashMap<String, Candle>>> = Lazy::new(|| {Mutex::new(HashMap::new())});

// When we receive a new candle,
// Either update the candle data (if it's the same)
// Or either send the old candle to the db and load the new candle into the hashmap
pub async fn proceed_data(new_candle: Candle) {
    // Load all the last candles from each symbol
    // Then select the right candle for the symbol
    let mut last_candles = CANDLES.lock().await;

    // Handle bug with empty symbol (binance)
    if  new_candle.symbol.is_empty() {
        return;
    }

    let last_candle = last_candles.get_mut(&new_candle.symbol).unwrap();
    
    // Check if it's the first candle
    // (The program has just started)
    // The open time being 0 means it 
    // or if the candle is the same as the last, 
    // we just update the candle
    if last_candle.open_time == 0 || last_candle.open_time == new_candle.open_time {
        *last_candle = new_candle.clone();
    } else {
        // If the open time is different, we first check the continuity
        // if the open time is not just superior by 1 milliscond to the last candle
        // we signal it 
        if new_candle.open_time - last_candle.close_time > 10000 {
            println!("Candle is not continuous: {:?} {:?}", new_candle, last_candle);
        } else {
            // If there is a continuity, we can update the close price
            last_candle.close = new_candle.open;
        }

        // Send the last candle to the websocket
        send_candle(&last_candle).await;

        // Then we send the candle to the db
        // And initialize the new candle
        add_candle(&last_candle.clone()).await;

        *last_candle = new_candle.clone();
    }


    // Send the new candle to the websocket
    send_candle(&last_candle).await;
}

pub async fn send_candle(candle: &Candle) {
    let mut data = Map::new();

    // Structure the data to send
    data.insert("type".to_string(), Value::String("candle".to_string()));
    data.insert("value".to_string(), serde_json::to_value(candle).unwrap());

    // Convert the data to a JSON string
    let json_data = Value::Object(data).to_string();

    // Send the data to the clients
    send_message_to_clients(&json_data).await;
}