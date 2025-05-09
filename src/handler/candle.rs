use common::{Candle, TIMERANGES};
use tokio::sync::Mutex;
use crate::server::{database::add_candle, websocket::send_message_to_clients};

use once_cell::sync::Lazy;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub enum CandleOrValue {
    Candle(Candle),
    Value(f64),
}

// Store all the candles of each symbol in a hashmap
// The key is the symbol and the value is the actual
pub static CANDLES: Lazy<Mutex<HashMap<String, HashMap<String, CandleOrValue>>>> = Lazy::new(|| {Mutex::new(HashMap::new())});

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

    // Define the volume to add
    let mut volume_to_add = 0.0;
    let mut usdt_volume_to_add = 0.0;

    // Load all the candles from the symbol
    // So we can update each timerange 
    let last_candles = last_candles.get_mut(&new_candle.symbol).unwrap();

    // Load the last volume and usdt volume
    let mut previous_volume = match last_candles.get("volume") {
        Some(CandleOrValue::Value(v)) => *v,
        _ => 0.0, 
    };
    let mut previous_usdt_volume = match last_candles.get("usdt_volume") {
        Some(CandleOrValue::Value(v)) => *v,
        _ => 0.0, 
    };

    for timerange in timeranges.iter() {
        // Get the last candle for the timerange
        if let Some(CandleOrValue::Candle(last_candle)) = last_candles.get_mut(timerange) {
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

                if timerange == "1m" {
                    // We need to initialize the volume and usdt volume
                    // Because we don't have the previous candles
                    volume_to_add = new_candle.volume;
                    usdt_volume_to_add = new_candle.usdt_volume;

                    previous_volume = 0.0;
                    previous_usdt_volume = 0.0;
                }
            } else if new_candle.open_time >= last_candle.open_time && new_candle.open_time < last_candle.close_time {
                // If the candle is in the same time range, we just update it
                // The open time and price are the same
                // We don't know the close price yet
                // So we just update the high and low, and actual price
                last_candle.low = last_candle.low.min(new_candle.low);
                last_candle.high = last_candle.high.max(new_candle.high);
                last_candle.price = new_candle.price;


                if timerange == "1m" {
                    // If the timerange is 1m, we need to update the volume and usdt volume
                    // So we know what we have to add to each candle
                    volume_to_add = new_candle.volume - previous_volume;
                    usdt_volume_to_add = new_candle.usdt_volume - previous_usdt_volume;                    
                }

                last_candle.volume += volume_to_add;
                last_candle.usdt_volume += usdt_volume_to_add;
            } else {
                // If the candle is not the same as the past one
                // We just create a new candle, and send the old one to the db
                if new_candle.open_time - last_candle.close_time > 1_0000 {
                    // If we notice a gap between the two candles we notify it
                    println!("Candle is not continuous: {:?} {:?}", new_candle, last_candle);
                } else {
                    // Before sending the new candle, we need to update the last candle
                    // The close time is the open time of the new candle
                    last_candle.close = Some(new_candle.open);
                }
                
                if timerange == "1m" {
                    // If there is a new candle, we reset the volume and usdt volume
                    volume_to_add = new_candle.volume;
                    usdt_volume_to_add = new_candle.usdt_volume;

                    previous_volume = 0.0;
                    previous_usdt_volume = 0.0;
                }

                // Update the volume and usdt volume
                last_candle.volume = volume_to_add;
                last_candle.usdt_volume = usdt_volume_to_add;

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

            // Send the new candle to the websocket
            send_candle(&last_candle).await;
        }

    }

    // We update the volume and usdt volume of the last candle
    last_candles.get_mut("volume").map(|v| {
        *v = CandleOrValue::Value(previous_volume + volume_to_add);
    });
    last_candles.get_mut("usdt_volume").map(|v| {
        *v = CandleOrValue::Value(previous_usdt_volume + usdt_volume_to_add);
    });

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