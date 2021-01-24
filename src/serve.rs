use crate::proto::orderbook;
use orderbook::orderbook_aggregator_server::OrderbookAggregator;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::watch::Receiver;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

#[derive(Clone)]
/// [OrderbookAggregator] server.
/// Responds to BookSummary requests with a stream of the values in `rx`.
pub struct Aggregator {
    rx: Receiver<Option<orderbook::Summary>>,
}

impl Aggregator {
    /// Returns a new [Aggregator] which will respond to rpc requests with a stream of the values in `rx`.
    pub fn new(rx: Receiver<Option<orderbook::Summary>>) -> Self {
        Self { rx }
    }
}

#[tonic::async_trait]
impl OrderbookAggregator for Aggregator {
    type BookSummaryStream =
        Pin<Box<dyn Stream<Item = Result<orderbook::Summary, Status>> + Send + Sync>>;
    async fn book_summary(
        &self,
        _: Request<orderbook::Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        let rx = self.rx.clone();
        Ok(Response::new(Box::pin(WatchStream::new(rx).map(Ok))))
    }
}

struct WatchStream {
    inputs: Receiver<Option<orderbook::Summary>>,
    next_future:
        Pin<Box<dyn Future<Output = Result<Option<orderbook::Summary>, ()>> + Send + Sync>>,
}

impl WatchStream {
    fn new(inputs: Receiver<Option<orderbook::Summary>>) -> Self {
        Self {
            next_future: Box::pin(recv_next(inputs.clone())),
            inputs,
        }
    }
}

impl Stream for WatchStream {
    type Item = orderbook::Summary;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.next_future.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(_)) => Poll::Ready(None),
            Poll::Ready(Ok(item)) => {
                self.next_future = Box::pin(recv_next(self.inputs.clone()));
                if let Some(item) = item {
                    Poll::Ready(Some(item))
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

async fn recv_next(
    mut rx: Receiver<Option<orderbook::Summary>>,
) -> Result<Option<orderbook::Summary>, ()> {
    rx.changed().await.map_err(|_| ())?;
    Ok(rx.borrow().as_ref().cloned())
}
