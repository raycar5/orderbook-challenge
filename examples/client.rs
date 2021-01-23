use orderbook::orderbook_aggregator_client::OrderbookAggregatorClient;
use orderbook_challenge::*;
use proto::orderbook;
use tonic::transport::Endpoint;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Endpoint::from_static("http://0.0.0.0:5005")
        .connect()
        .await?;

    let mut orderbook_client = OrderbookAggregatorClient::new(channel.clone());

    let request = tonic::Request::new(orderbook::Empty {});

    let mut response = orderbook_client.book_summary(request).await?.into_inner();

    while let Ok(Some(summary)) = response.message().await {
        println!("{:#?}", summary)
    }

    Ok(())
}
