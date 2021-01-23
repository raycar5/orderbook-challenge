use super::super::{DeserializeArrayVec, Exchange, InputUpdate, Level};
use crate::TOP_LEVELS;
use async_stream::stream;
use backoff::{backoff::Backoff, tokio::retry_notify};
use serde::Deserialize;
use std::borrow::Cow;
use tokio_stream::{Stream, StreamExt};
use tokio_tungstenite::connect_async;
use tungstenite::Message;
use url::Url;

#[derive(Deserialize)]
/// Represents websocket messages from Binance.
struct BinanceInput {
    asks: DeserializeArrayVec<[Level; TOP_LEVELS]>,
    bids: DeserializeArrayVec<[Level; TOP_LEVELS]>,
}

impl Into<InputUpdate> for BinanceInput {
    fn into(self) -> InputUpdate {
        let BinanceInput { asks, bids } = self;

        // We assume that asks and bids come sorted from Binance,
        // this call will panic in `debug` mode if that is not the case.
        InputUpdate::new(Exchange::Binance, asks.into(), bids.into())
    }
}

/// Establishes a new connection to Binance and returns a [Stream] of [BinanceInput].
async fn get_stream_inner<B: Backoff>(
    url: Url,
    // Backoff is not Clone.
    backoff: impl Fn() -> B,
) -> impl Stream<Item = Result<BinanceInput, tungstenite::Error>> {
    retry_notify(
        backoff(),
        || async {
            let (socket, _) = connect_async(url.clone()).await?;
            Ok(socket)
        },
        |err, _| eprintln!("Error creating Binance connection: {}, retrying", err),
    )
    .await
    .expect("Could not open connection to Binance")
    .filter_map(|item| {
        match item {
            Ok(Message::Text(mut text)) => match simd_json::from_str::<BinanceInput>(&mut text) {
                Ok(input) => Some(Ok(input)),
                Err(err) => Some(Err(tungstenite::Error::Protocol(Cow::Owned(
                    err.to_string(),
                )))),
            },
            Ok(Message::Close(_)) => Some(Err(tungstenite::Error::ConnectionClosed)),
            Err(err) => Some(Err(err)),
            Ok(_) => {
                // Ignore ping, pong and binary messages
                None
            }
        }
    })
}

/// Creates a new [InputUpdate] [Stream] from the provided `pair` by connecting to the Binance [websocket API](https://github.com/binance/binance-spot-api-docs/blob/master/web-socket-streams.md#partial-book-depth-streams).
/// The stream is resilient and will retry if errors happen. If the pair is not valid, the stream will be empty, this is because there is no signal
// from Binance that indicates a pair is not valid.
pub fn get_stream<B: Backoff>(
    pair: String,
    backoff: impl Fn() -> B + Clone,
) -> impl Stream<Item = InputUpdate> {
    let url = Url::parse(&format!(
        "wss://stream.binance.com:9443/ws/{}@depth10@100ms",
        pair
    ))
    .expect("Invalid pair");

    stream! {
        loop{
            let mut s = get_stream_inner(url.clone(),backoff.clone()).await;
            while let Some(value) = s.next().await {
                match value {
                    Ok(update) => yield update.into(),
                    Err(err) => {
                        eprintln!("Unexpected error in Binance stream: {}, restarting",err);
                        s = get_stream_inner(url.clone(),backoff.clone()).await;
                    }
                }
            }
            eprintln!("Binance stream stopped unexpectedly, restarting");
        }
    }
}
