# Orderbook Challenge
Code challenge to implement a simple orderbook aggregator that takes the top 10 orders from 2 crypto exchanges
and exposes a grpc endpoint which streams the sorted and merged orderbook and spread.

## Running

- Server: `PAIR=ethbtc cargo run --release --example server`
- Client: `cargo run --release --example client`

## Docs
You can generate documentation by running `doc.sh`, it will automatically open in a browser tab (on systems with `xdg-open`).

## Profiling
You can generate a flamegraph profile of the server by running `profile.sh`, it requires that you have previously ran `cargo install flamegraph`.

## Testing
Run `cargo test` to execute unit tests.

## Decision Notes

- Pairs are not validated, neither Bitstamp nor Binance return errors when a provided trading pair is invalid, the solution could be a local dictionary of pairs but I thought it would be unnecessary.
- The parsers assume that the websocket endpoints provide a sorted orderbook, this is checked in `debug` mode but not in `release` mode.

## Next steps

If this was a production system, I would take the following steps to improve it:

1. Integration testing with mock ws endpoints. 
2. Performance tracing for end-to-end latency.
3. Switch to a local orderbook and diff based processing. This should only be done once the previous step is finished because switching to diff based processing greatly increases complexity, especially in the retry logic, and in order to know if it is worth it, we need the tracing system to measure the improvement.
4. Dependency audit.
5. Structured logging and metrics.
6. Many more cycles of measure->investigate->optimize.
