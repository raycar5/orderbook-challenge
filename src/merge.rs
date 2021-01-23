use crate::{
    input::{Exchange, InputUpdate, Level},
    proto::orderbook,
    TOP_LEVELS,
};
use arrayvec::ArrayVec;
use async_stream::stream;
use std::cmp::Ordering;
use std::convert::TryInto;
use tokio::sync::mpsc::Receiver;
use tokio_stream::Stream;

/// Returns a stream of [orderbook::Summary] which emits whenever a new [InputUpdate] is received through `inputs`.
pub fn merge(mut inputs: Receiver<InputUpdate>) -> impl Stream<Item = orderbook::Summary> {
    let mut state = MergeState::new();
    stream! {
        while let Some(input) = inputs.recv().await{
            state.update(input);
            yield state.summary();
        }
    }
}

#[derive(Debug)]
/// Stores the latest updates from every [Exchange] and provides [MergeState::summary]
/// to merge them into on [orderbook::Summary].
struct MergeState {
    asks: [ArrayVec<[Level; TOP_LEVELS]>; Exchange::VARIANT_COUNT],
    bids: [ArrayVec<[Level; TOP_LEVELS]>; Exchange::VARIANT_COUNT],
}
impl MergeState {
    /// Returns a new empty [MergeState].
    fn new() -> Self {
        Self {
            asks: Default::default(),
            bids: Default::default(),
        }
    }

    /// Updates the latest asks and bids for an exchange.
    fn update(&mut self, input: InputUpdate) {
        let (exchange, asks, bids) = input.take();

        self.asks[exchange as usize] = asks;
        self.bids[exchange as usize] = bids;
    }

    /// Returns a new [orderbook::Summary] with the top [TOP_LEVELS] asks and bids from each [Exchange].
    fn summary(&self) -> orderbook::Summary {
        let asks = calculate_levels(
            &self.asks,
            Level::cmp_ask,
            TOP_LEVELS * Exchange::VARIANT_COUNT,
        );

        let bids = calculate_levels(
            &self.bids,
            Level::cmp_bid,
            TOP_LEVELS * Exchange::VARIANT_COUNT,
        );

        let spread = if asks.is_empty() || bids.is_empty() {
            0.
        } else {
            asks[0].price - bids[0].price
        };
        orderbook::Summary { asks, bids, spread }
    }
}

/// Returns a sorted [Vec] of `size` from the levels in `exchanges`.
///
/// This implementation uses naive linear search, since [TOP_LEVELS] is small,
/// and the majority of the overhead is in IO and parsing, this function doesn't
/// even show up in the flamegraph.
///
/// If [TOP_LEVELS] increases the implementation could be switched with a heap based implementation.
/// Although it's likely that a better idea would be to switch the whole pipeline to operate on diffs.
fn calculate_levels(
    exchanges: &[ArrayVec<[Level; TOP_LEVELS]>; Exchange::VARIANT_COUNT],
    cmp_fn: impl Fn(&Level, &Level) -> Ordering,
    size: usize,
) -> Vec<orderbook::Level> {
    let mut output = Vec::<orderbook::Level>::new();
    output.reserve(size);
    for (exchange, levels) in exchanges.iter().enumerate() {
        for level in levels {
            if output.is_empty() {
                output.push(level.into_orderbook_level((exchange as u8).try_into().unwrap()));
                continue;
            }

            let reverse_output = output.iter().enumerate().rev();
            let mut insert_index = None;

            for (i, rev_out) in reverse_output {
                if matches!(cmp_fn(&level, &rev_out.try_into().unwrap()), Ordering::Less) {
                    insert_index = Some(i);
                } else if output.len() < size {
                    insert_index = Some(i + 1);
                    break;
                } else {
                    break;
                }
            }

            if let Some(i) = insert_index {
                if output.len() >= size {
                    output.pop();
                }
                output.insert(
                    i,
                    level.into_orderbook_level(
                        (exchange as u8)
                            .try_into()
                            .expect("exchange should be within 0..Exchange::VARIANT_COUNT"),
                    ),
                )
            }
        }
    }
    output
}

#[cfg(test)]
mod test {
    use crate::{input::Exchange, is_sorted};
    use quickcheck_macros::quickcheck;

    use super::*;

    #[test]
    fn test_bids() {
        // Best.
        assert_eq!(
            &calculate_levels(
                &[
                    arrayvec![lvl!(50., 1.), lvl!(40., 1.)],
                    arrayvec![lvl!(51., 1.), lvl!(30., 1.)]
                ],
                Level::cmp_bid,
                2
            ),
            &[lvl1!(51., 1.), lvl0!(50., 1.)]
        );

        // Equal price levels.
        assert_eq!(
            &calculate_levels(
                &[
                    arrayvec![lvl!(51., 3.), lvl!(51., 1.)],
                    arrayvec![lvl!(51., 2.), lvl!(51., 1.)]
                ],
                Level::cmp_bid,
                2
            ),
            &[lvl0!(51., 3.), lvl1!(51., 2.)]
        );

        // Bigger size.
        assert_eq!(
            &calculate_levels(
                &[
                    arrayvec![lvl!(51., 3.), lvl!(51., 1.)],
                    arrayvec![lvl!(51., 2.), lvl!(51., 1.)]
                ],
                Level::cmp_bid,
                3
            ),
            &[lvl0!(51., 3.), lvl1!(51., 2.), lvl0!(51., 1.)]
        );
    }

    #[test]
    fn test_asks() {
        // Best.
        assert_eq!(
            &calculate_levels(
                &[
                    arrayvec![lvl!(50., 1.), lvl!(40., 1.)],
                    arrayvec![lvl!(51., 1.), lvl!(30., 1.)]
                ],
                Level::cmp_ask,
                2
            ),
            &[lvl1!(30., 1.), lvl0!(40., 1.)]
        );

        // Equal price levels.
        assert_eq!(
            &calculate_levels(
                &[
                    arrayvec![lvl!(51., 3.), lvl!(51., 1.)],
                    arrayvec![lvl!(51., 2.), lvl!(51., 1.)]
                ],
                Level::cmp_ask,
                2
            ),
            &[lvl0!(51., 3.), lvl1!(51., 2.)]
        );

        // Bigger size.
        assert_eq!(
            &calculate_levels(
                &[
                    arrayvec![lvl!(51., 3.), lvl!(51., 1.)],
                    arrayvec![lvl!(51., 2.), lvl!(51., 1.)]
                ],
                Level::cmp_ask,
                3
            ),
            &[lvl0!(51., 3.), lvl1!(51., 2.), lvl0!(51., 1.)]
        );
    }

    #[quickcheck]
    fn test_stays_sorted(inputs: Vec<InputUpdate>) {
        let mut state = MergeState::new();
        // Nothin sus happenin here.
        println!("state:{:?}", state);
        for update in inputs {
            state.update(update);
            let orderbook::Summary { asks, bids, spread } = state.summary();
            assert_eq!(
                if !asks.is_empty() && !bids.is_empty() {
                    asks[0].price - bids[0].price
                } else {
                    0.
                },
                spread
            );
            let asks: Vec<Level> = asks
                .iter()
                .map(TryInto::try_into)
                .map(Result::unwrap)
                .collect();
            let bids: Vec<Level> = bids
                .iter()
                .map(TryInto::try_into)
                .map(Result::unwrap)
                .collect();
            assert!(is_sorted(&asks, Level::cmp_ask), "asks: {:?}", asks);
            assert!(is_sorted(&bids, Level::cmp_bid), "bids: {:?}", bids);
        }
    }
    use better_macro::println;
}
