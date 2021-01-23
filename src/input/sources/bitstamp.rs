use super::super::{DeserializeArrayVec, Exchange, InputUpdate, Level};
use crate::TOP_LEVELS;
use async_stream::stream;
use backoff::{backoff::Backoff, tokio::retry_notify};
use futures_util::SinkExt;
use serde::Deserialize;
use std::borrow::Cow;
use tokio_stream::{Stream, StreamExt};
use tokio_tungstenite::connect_async;
use tungstenite::Message;
use url::Url;

#[derive(Deserialize)]
/// Represents data inside Bitstamp `data` messages.
struct BitstampData {
    asks: DeserializeArrayVec<[Level; TOP_LEVELS]>,
    bids: DeserializeArrayVec<[Level; TOP_LEVELS]>,
}

#[derive(Deserialize)]
#[serde(tag = "event")]
#[serde(rename_all = "lowercase")]
/// Represents websocket messages from Bitstamp.
enum BitstampInput {
    Data {
        data: BitstampData,
    },
    #[serde(rename = "bts:request_reconnect")]
    Reconnect,
    #[serde(rename = "bts:subscription_succeeded")]
    SubSuccess,
}

impl Into<InputUpdate> for BitstampInput {
    fn into(self) -> InputUpdate {
        if let BitstampInput::Data {
            data: BitstampData { asks, bids },
        } = self
        {
            // We assume that asks and bids come sorted from Bitstamp,
            // this call will panic in `debug` mode if that is not the case.
            InputUpdate::new(Exchange::Bitstamp, asks.into(), bids.into())
        } else {
            unreachable!("unhandled reconnect packet")
        }
    }
}

/// Establishes a new connection to Bitstamp and returns a [Stream] of [BitstampInput].
async fn get_stream_inner<B: Backoff>(
    subscribe_message: String,
    // Backoff is not Clone.
    backoff: impl Fn() -> B,
) -> impl Stream<Item = Result<BitstampInput, tungstenite::Error>> {
    let url = Url::parse("wss://ws.bitstamp.net").unwrap();

    retry_notify(
        backoff(),
        || async {
            let (mut socket, _) = connect_async(url.clone()).await?;
            socket.send(subscribe_message.clone().into()).await?;
            Ok(socket)
        },
        |err, _| eprintln!("Error creating Bitstamp connection: {}, retrying", err),
    )
    .await
    .expect("Could not open connection to Bitstamp")
    .filter_map(|item| {
        match item {
            Ok(Message::Text(mut text)) => match simd_json::from_str::<BitstampInput>(&mut text) {
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

/// Creates a new [InputUpdate] [Stream] from the provided `pair` by connecting to the Bitstamp [websocket API](https://www.bitstamp.net/websocket/v2/).
/// The stream is resilient and will retry if errors happen. If the pair is not valid, the stream will be empty, this is because there is no signal
// from Bitstamp that indicates a pair is not valid.
pub fn get_stream<B: Backoff>(
    pair: String,
    backoff: impl Fn() -> B + Clone,
) -> impl Stream<Item = InputUpdate> {
    let subscribe_message = format!(
        r#"
        {{
            "event": "bts:subscribe",
            "data": {{
                "channel": "order_book_{}"
            }}
        }}
        "#,
        pair
    );

    stream! {
        loop{
            let mut s = get_stream_inner(subscribe_message.clone(),backoff.clone()).await;

            while let Some(value) = s.next().await {
                match value{
                    Ok(value @BitstampInput::Data{..}) => yield value.into(),
                    Ok(BitstampInput::Reconnect)=>{
                        eprintln!("Reconnect request received from Bitstamp, reconnecting");
                        s = get_stream_inner(subscribe_message.clone(),backoff.clone()).await;
                    }
                    Err(err)=>{
                        eprintln!("Unexpected error in Bitstamp stream: {}, restarting",err);
                        s = get_stream_inner(subscribe_message.clone(),backoff.clone()).await;
                    }
                    Ok(BitstampInput::SubSuccess) => {
                        // Ignore successful connection message.
                    }
                }
            }
            eprintln!("Bitstamp stream stopped unexpectedly, restarting");
        }
    }
}
