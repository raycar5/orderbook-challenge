use super::super::{DeserializeArrayVec, Exchange, InputUpdate, Level};
use crate::TOP_LEVELS;
use backoff::{backoff::Backoff, tokio::retry_notify};
use futures_util::SinkExt;
use serde::Deserialize;
use std::{
    borrow::Cow,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
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

type StreamType = Pin<Box<dyn Stream<Item = Result<BitstampInput, tungstenite::Error>> + Send>>;
/// Establishes a new connection to Bitstamp and returns a [Stream] of [BitstampInput].
async fn get_stream_inner<B: Backoff>(
    subscribe_message: String,
    // Backoff is not Clone.
    backoff: impl Fn() -> B,
) -> StreamType {
    let url = Url::parse("wss://ws.bitstamp.net").unwrap();

    Box::pin(
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
                Ok(Message::Text(mut text)) => {
                    match simd_json::from_str::<BitstampInput>(&mut text) {
                        Ok(input) => Some(Ok(input)),
                        Err(err) => Some(Err(tungstenite::Error::Protocol(Cow::Owned(
                            err.to_string(),
                        )))),
                    }
                }
                Ok(Message::Close(_)) => Some(Err(tungstenite::Error::ConnectionClosed)),
                Err(err) => Some(Err(err)),
                Ok(_) => {
                    // Ignore ping, pong and binary messages
                    None
                }
            }
        }),
    )
}

enum BitstampStreamState {
    // If only there was a macro that allowed me to describe this without
    // dynamic dispatch and in a more readable way.
    FetchingStream(Pin<Box<dyn Future<Output = StreamType> + Send>>),
    Streaming(StreamType),
}

pub struct BitstampStream<B: Backoff + Send, F: Fn() -> B + Clone + Send> {
    state: BitstampStreamState,
    backoff: F,
    subscribe_message: String,
}
impl<B: Backoff + 'static + Send, F: Fn() -> B + Clone + Send + 'static + Sync>
    BitstampStream<B, F>
{
    /// Creates a new [InputUpdate] [Stream] from the provided `pair` by connecting to the Bitstamp [websocket API](https://www.bitstamp.net/websocket/v2/).
    /// The stream is resilient and will retry if errors happen. If the pair is not valid, the stream will be empty, this is because there is no signal
    // from Bitstamp that indicates a pair is not valid.
    pub fn new(pair: String, backoff: F) -> Self {
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
        Self {
            state: BitstampStreamState::FetchingStream(Box::pin(get_stream_inner(
                subscribe_message.clone(),
                backoff.clone(),
            ))),
            backoff,
            subscribe_message,
        }
    }
}

impl<B: Backoff + 'static + Unpin + Send, F: Fn() -> B + Clone + 'static + Unpin + Send + Sync>
    Stream for BitstampStream<B, F>
{
    type Item = InputUpdate;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.state {
            BitstampStreamState::FetchingStream(ref mut fut) => match fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(stream) => {
                    self.state = BitstampStreamState::Streaming(stream);
                    self.poll_next(cx)
                }
            },
            BitstampStreamState::Streaming(ref mut stream) => {
                match stream.as_mut().poll_next(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Some(Ok(data @ BitstampInput::Data { .. }))) => {
                        Poll::Ready(Some(data.into()))
                    }
                    Poll::Ready(Some(Ok(BitstampInput::Reconnect))) => {
                        eprintln!("Reconnect request received from Bitstamp, reconnecting");
                        self.state = BitstampStreamState::FetchingStream(Box::pin(
                            get_stream_inner(self.subscribe_message.clone(), self.backoff.clone()),
                        ));
                        self.poll_next(cx)
                    }
                    Poll::Ready(Some(Ok(BitstampInput::SubSuccess))) => {
                        // Ignore successful connection message.
                        self.poll_next(cx)
                    }
                    Poll::Ready(Some(Err(err))) => {
                        eprintln!("Unexpected error in Bitstamp stream: {}, restarting", err);
                        self.state = BitstampStreamState::FetchingStream(Box::pin(
                            get_stream_inner(self.subscribe_message.clone(), self.backoff.clone()),
                        ));
                        self.poll_next(cx)
                    }
                    Poll::Ready(None) => {
                        eprintln!("Bitstamp stream stopped unexpectedly, restarting");
                        self.state = BitstampStreamState::FetchingStream(Box::pin(
                            get_stream_inner(self.subscribe_message.clone(), self.backoff.clone()),
                        ));
                        self.poll_next(cx)
                    }
                }
            }
        }
    }
}
