use common::{Candle, TIMERANGES};
use tokio::sync::Mutex;
use crate::server::{database::add_candle, websocket::send_message_to_clients};

use once_cell::sync::Lazy;
use serde_json::{Map, Value};
use std::collections::HashMap;

// Store all the candles of each symbol in a hashmap
// The key is the symbol and the value is the actual
pub static CANDLES: Lazy<Mutex<HashMap<String, HashMap<String, Candle>>>> = Lazy::new(|| {Mutex::new(HashMap::new())});

// When we receive a new candle,
// Either update the candle data (if it's the same)
// Or either send the old candle to the db and load the new candle into the hashmap
pub async fn proceed_data(new_candle: Candle) {
    // Load the last candles from the hashmap
    let mut last_candles = CANDLES.lock().await;

    // Handle bug with empty symbol (binance)
    if  new_candle.symbol.is_empty() {
        return;
    }

    // Load the different timeranges
    let timeranges = {
        let timeranges = TIMERANGES.lock().await;
        timeranges.clone()
    };

    // Load all the candles from the symbol
    // So we can update each timerange 
    let last_candles = last_candles.get_mut(&new_candle.symbol).unwrap();
    for timerange in timeranges {
        // Get the last candle for the timerange
        let last_candle = last_candles.get_mut(&timerange).unwrap();

        // If the last candle is empty, we just load the new candle
        if last_candle.open_time == 0{
            // We can't gurantee a good open price, low and high 
            // But for now we do it like this
            *last_candle = new_candle.clone();

            // Set the open time and close time
            // Respecting the timerange
            last_candle.timerange = timerange.clone();
            let (open_time, close_time) = get_timerange(&timerange, new_candle.open_time);
            last_candle.open_time = open_time;
            last_candle.close_time = close_time;
        } else if new_candle.open_time >= last_candle.open_time && new_candle.open_time < last_candle.close_time {
            // If the candle is in the same time range, we just update it
            // The open time and price are the same
            // We don't know the close price yet
            // With binance, the close price is the actual price
            // So we just update the high and low, and actual price
            last_candle.low = last_candle.low.min(new_candle.low);
            last_candle.high = last_candle.high.max(new_candle.high);
            last_candle.price = new_candle.price;
        } else {
            // If the candle is not the same as the past one
            // We just create a new candle, and send the old one to the db
            if new_candle.open_time - last_candle.close_time > 10000 {
                // If we notice a gap between the two candles we notify it
                println!("Candle is not continuous: {:?} {:?}", new_candle, last_candle);
            } else {
                // Before sending the new candle, we need to update the last candle
                // The close time is the open time of the new candle
                last_candle.close = Some(new_candle.open);
            }

            // Send the last candle to the websocket
            send_candle(&last_candle).await;

            // Send the last candle to the db
            add_candle(&last_candle.clone()).await;

            // Update the last candle with the new one
            *last_candle = new_candle.clone();

            // Set the open time and close time
            // Respecting the timerange
            last_candle.timerange = timerange.clone();
            let (open_time, close_time) = get_timerange(&timerange, new_candle.open_time);
            last_candle.open_time = open_time;
            last_candle.close_time = close_time;
        }

        // Send thecandle to the websocket
        send_candle(&last_candle).await;
    }
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

// This function will determine the open time and close time of the candle
// depending on the timerange
// because we can't create timeranges on the fly we need to calculate the open and close time
// Actual time is the current time in nanoseconds
pub fn get_timerange(timerange: &str, actual_time_ms: i64) -> (i64, i64) {
    let duration_ms = match timerange {
        "1m" => 60_000,
        "5m" => 5 * 60_000,
        "15m" => 15 * 60_000,
        "30m" => 30 * 60_000,
        "1h" => 60 * 60_000,
        "4h" => 4 * 60 * 60_000,
        "1d" => 24 * 60 * 60_000,
        _ => {
            eprintln!("Unknown timerange: {}", timerange);
            return (0, 0);
        }
    };

    let open_time = actual_time_ms - (actual_time_ms % duration_ms);

    // The close time is the open time + duration - 1 second
    // We subtract 1 second to avoid having the same open and close time
    let close_time = open_time + duration_ms - 1_000;
    (open_time, close_time)
}