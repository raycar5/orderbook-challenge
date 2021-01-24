use input::*;
use orderbook_challenge::*;
use proto::orderbook::orderbook_aggregator_server::OrderbookAggregatorServer;
use serve::Aggregator;
use tokio::{
    spawn,
    sync::{mpsc, watch},
};
use tokio_stream::StreamExt;
use tonic::transport::Server;

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel(CHANNEL_SIZE);
    let tx_c = tx.clone();

    let pair = std::env::var("PAIR").expect(
        "Please provide a trading pair in the PAIR environment variable for example: PAIR=ethbtc",
    );
    let pair_c = pair.clone();

    // Spawn Bitstamp task.
    spawn(async move {
        let stream =
            sources::bitstamp::BitstampStream::new(pair, || backoff::ExponentialBackoff::default());
        tokio::pin!(stream);
        while let Some(item) = stream.next().await {
            tx_c.send(item).await.unwrap();
        }
    });

    // Spawn Binance task.
    spawn(async move {
        let tx = tx.clone();
        let stream =
            sources::binance::BinanceStream::new(pair_c, || backoff::ExponentialBackoff::default());
        tokio::pin!(stream);
        while let Some(item) = stream.next().await {
            tx.send(item).await.unwrap();
        }
    });

    let (summaries_tx, summaries_rx) = watch::channel(None);
    // Spawn merge task.
    spawn(async move {
        let stream = merge::merge(rx);
        tokio::pin!(stream);
        while let Some(item) = stream.next().await {
            summaries_tx.send(Some(item)).expect("Watch channel broke!");
        }
    });

    // Start server.
    Server::builder()
        .add_service(OrderbookAggregatorServer::new(Aggregator::new(
            summaries_rx,
        )))
        .serve("0.0.0.0:5005".parse().unwrap())
        .await
        .unwrap();
}
