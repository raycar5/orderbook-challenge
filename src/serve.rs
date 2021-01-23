use crate::proto::orderbook;
use async_stream::stream;
use orderbook::orderbook_aggregator_server::OrderbookAggregator;
use std::pin::Pin;
use tokio::sync::watch::Receiver;
use tokio_stream::Stream;
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
        let mut rx = self.rx.clone();

        Ok(Response::new(Box::pin(stream! {
            while let Ok(_) = rx.changed().await{
                let cloned = rx.borrow().clone();
                if let Some(summary) = cloned{
                    yield Ok(summary)
                }
            }
        })))
    }
}
