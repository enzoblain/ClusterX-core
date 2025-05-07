use common::Candle;
use serde_json::Value;

pub fn parse_message(message: &str) -> Option<Candle> {
    // Parse the JSON message from Binance
    // The data we want is in the "k" field whichi is in the "data" field
    let parsed: Value = serde_json::from_str(message).ok()?;
    let data = &parsed["data"];
    let kline = &data["k"];

    // Extract the relevant fields from the kline object
    // And convert them to the appropriate types
    let open_time = kline["t"].as_i64()?;
    let close_time = kline["T"].as_i64()?;
    let symbol = kline["s"].as_str()?.to_string();
    let interval = kline["i"].as_str()?.to_string();
    let open = kline["o"].as_str()?.parse::<f64>().ok()?;
    let price = kline["c"].as_str()?.parse::<f64>().ok()?;
    let high = kline["h"].as_str()?.parse::<f64>().ok()?;
    let low = kline["l"].as_str()?.parse::<f64>().ok()?;
    let volume = kline["v"].as_str()?.parse::<f64>().ok()?;

    // Create a Candle object with the extracted data
    // and return it
    Some(Candle {
        open_time,
        close_time,
        symbol,
        interval,
        open,
        close: 0.0, // Because the close is the actual price
        high,
        low,
        price: price.into(),
        volume,
    })
}