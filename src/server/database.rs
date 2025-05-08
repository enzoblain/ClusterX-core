use common::DB_CLIENT;
use crate::handler::candle::{CANDLES, CandleOrValue};

use chrono::{DateTime, Utc};
use common::{Candle, TIMERANGES};
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

// Facilitate access to the database client
pub async fn get_db_client() -> Arc<Mutex<tokio_postgres::Client>> {
    DB_CLIENT.get().expect("Database client not initialized").clone()
}

pub async fn load_last_candles(symbols: Vec<String>) {
    let client = get_db_client().await;
    let client = client.lock().await;

    // Load all timeranges
    let timeranges = {
        let timeranges = TIMERANGES.lock().await;
        timeranges.clone() 
    };
    
    // Initialize the hashmap with the timeranges
    // and the symbols
    // Iterate over the symbols and create a hashmap for each symbol
    // Then create a hashmap for each timerange in the symbol
    // Add also volume entries to have a live volume
    let mut new_entries = HashMap::new();
    for symbol in &symbols {
        let mut map = timeranges
            .iter()
            .map(|t| (t.clone(), CandleOrValue::Candle(Candle::default())))
            .collect::<HashMap<_, _>>();

        map.insert("volume".to_string(), CandleOrValue::Value(0.0));
        map.insert("usdt_volume".to_string(), CandleOrValue::Value(0.0));

        new_entries.insert(symbol.clone(), map);
    }

    // Prepare the SQL query to fetch the last candles for the given symbols
    // Timerange of 1m because we are using 1m candles (binance) 
    // This should change depending on the provider
    let query = "SELECT DISTINCT ON (symbol, timerange) * FROM candles WHERE symbol = ANY($1) ORDER BY symbol, timerange, open_time DESC";
    let symbols: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
    let rows = client.query(query, &[&symbols]).await.expect("Failed to fetch last candles");

    for last_candle in rows {
        let symbol: String = last_candle.get("symbol");
        let open_time: DateTime<Utc> = last_candle.get("open_time");
        let open_time_unix: i64 = open_time.timestamp_millis();
        let close_time: DateTime<Utc> = last_candle.get("close_time");
        let close_time_unix: i64 = close_time.timestamp_millis();
        let timerange: String = last_candle.get("timerange");
        let open: f64 = last_candle.get("open");
        let high: f64 = last_candle.get("high");
        let low: f64 = last_candle.get("low");
        let price: f64 = last_candle.get("close");
        let volume: f64 = last_candle.get("volume");

        // If the candle is in the hashmap, update it
        // Other wise we just ignore it
        if  let Some(CandleOrValue::Candle(candle)) = new_entries.get_mut(&symbol) .and_then(|m| m.get_mut(&timerange)) {
            candle.open_time = open_time_unix;
            candle.close_time = close_time_unix;
            candle.open = open;
            candle.high = high;
            candle.low = low;
            candle.price = Some(price);
            candle.close = None;
            candle.volume = volume;
        }
    }

    // Now we can update the CANDLES hashmap with the new entries
    let mut candles = CANDLES.lock().await;
    candles.extend(new_entries);
}

pub async fn add_candle(candle: &Candle) {
    let client = get_db_client().await;
    let client = client.lock().await;

    // Convert the open and close times to timestamps
    // The timestamps are in milliseconds, so we divide by 1000
    let open_time = DateTime::<Utc>::from_timestamp(candle.open_time / 1000, 0).unwrap();
    let close_time = DateTime::<Utc>::from_timestamp(candle.close_time / 1000, 0).unwrap();

    // We are sure that the close price is not None
    // So we can unwrap it
    let usdt_volume = candle.volume * candle.close.unwrap();

    // Prepare the SQL query to insert the candle into the database
    let query = "INSERT INTO candles (symbol, timerange, open_time, close_time, open, high, low, close, volume, usdt_volume) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (symbol, timerange, open_time) DO UPDATE SET high = EXCLUDED.high, low = EXCLUDED.low, close = EXCLUDED.close, volume = EXCLUDED.volume, usdt_volume = EXCLUDED.usdt_volume;";
    client.execute(query, &[&candle.symbol, &candle.timerange, &open_time, &close_time, &candle.open, &candle.high, &candle.low, &candle.price, &candle.volume, &usdt_volume]).await.expect("Failed to insert candle into the database");

}