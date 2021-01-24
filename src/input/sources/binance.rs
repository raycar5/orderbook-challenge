use super::super::{DeserializeArrayVec, Exchange, InputUpdate, Level};
use crate::TOP_LEVELS;
use backoff::{backoff::Backoff, tokio::retry_notify};
use serde::Deserialize;
use std::{borrow::Cow, future::Future};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
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

type StreamType = Pin<Box<dyn Stream<Item = Result<BinanceInput, tungstenite::Error>> + Send>>;
/// Establishes a new connection to Binance and returns a [Stream] of [BinanceInput].
async fn get_stream_inner<B: Backoff>(
    url: Url,
    // Backoff is not Clone.
    backoff: impl Fn() -> B,
) -> StreamType {
    Box::pin(
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
                Ok(Message::Text(mut text)) => match simd_json::from_str::<BinanceInput>(&mut text)
                {
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
        }),
    )
}

enum BinanceStreamState {
    // If only there was a macro that allowed me to describe this without
    // dynamic dispatch and in a more readable way.
    FetchingStream(Pin<Box<dyn Future<Output = StreamType> + Send>>),
    Streaming(StreamType),
}

pub struct BinanceStream<B: Backoff + Send, F: Fn() -> B + Clone + Send> {
    state: BinanceStreamState,
    backoff: F,
    url: Url,
}
impl<B: Backoff + 'static + Send, F: Fn() -> B + Clone + Send + 'static + Sync>
    BinanceStream<B, F>
{
    /// Creates a new [InputUpdate] [Stream] from the provided `pair` by connecting to the Binance [websocket API](https://github.com/binance/binance-spot-api-docs/blob/master/web-socket-streams.md#partial-book-depth-streams).
    /// The stream is resilient and will retry if errors happen. If the pair is not valid, the stream will be empty, this is because there is no signal
    // from Binance that indicates a pair is not valid.
    pub fn new(pair: String, backoff: F) -> Self {
        let url = Url::parse(&format!(
            "wss://stream.binance.com:9443/ws/{}@depth10@100ms",
            pair
        ))
        .expect("Invalid pair");

        Self {
            state: BinanceStreamState::FetchingStream(Box::pin(get_stream_inner(
                url.clone(),
                backoff.clone(),
            ))),
            backoff,
            url,
        }
    }
}
impl<B: Backoff + 'static + Unpin + Send, F: Fn() -> B + Clone + 'static + Unpin + Send + Sync>
    Stream for BinanceStream<B, F>
{
    type Item = InputUpdate;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.state {
            BinanceStreamState::FetchingStream(ref mut fut) => match fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(stream) => {
                    self.state = BinanceStreamState::Streaming(stream);
                    self.poll_next(cx)
                }
            },
            BinanceStreamState::Streaming(ref mut stream) => {
                match stream.as_mut().poll_next(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Some(Ok(input))) => Poll::Ready(Some(input.into())),
                    Poll::Ready(Some(Err(err))) => {
                        eprintln!("Unexpected error in Binance stream: {}, restarting", err);
                        self.state = BinanceStreamState::FetchingStream(Box::pin(
                            get_stream_inner(self.url.clone(), self.backoff.clone()),
                        ));
                        self.poll_next(cx)
                    }
                    Poll::Ready(None) => {
                        eprintln!("Binance stream stopped unexpectedly, restarting");
                        self.state = BinanceStreamState::FetchingStream(Box::pin(
                            get_stream_inner(self.url.clone(), self.backoff.clone()),
                        ));
                        self.poll_next(cx)
                    }
                }
            }
        }
    }
}
