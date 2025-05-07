use common::Candle;

use super::binance;

// This function constructs a WebSocket URL for a given provider and symbols.
// Each provider has its own URL format
// and the function handles the construction based on the provider's requirements.
pub fn build_stream_url(provider: &str, symbols: &[String], stream_type: &str) -> String {
    match provider {
        "Binance" => {
            // Construct the WebSocket URL for Binance
            // The URL format is wss://stream.binance.com:9443/stream?streams=<symbol1>@<stream_type>/<symbol2>@<stream_type>...
            let symbol_list = symbols.iter()
                .map(|symbol| format!("{}@{}", symbol.to_lowercase(), stream_type))
                .collect::<Vec<_>>()
                .join("/");

            format!("wss://stream.binance.com:9443/stream?streams={}", symbol_list)
        },
        _ => {
            eprintln!("Unsupported provider: {}", provider);
            String::new()
        }
    }
}

// This function returns a parsed Candle object from the message received from the WebSocket stream
// Each provider has its own message format
// and the function handles the parsing based on the provider's requirements.
pub fn parse_candle(message: &str, provider: &str) -> Option<Candle> {
    match provider {
        "Binance" => binance::parse_message(message),
        _ => {
            eprintln!("Unsupported provider: {}", provider);
            None
        }
    }
}