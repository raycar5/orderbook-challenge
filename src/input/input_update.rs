use super::{Exchange, Level};
use crate::{is_sorted, proto::orderbook, TOP_LEVELS};
use arrayvec::ArrayVec;
#[cfg(test)]
use quickcheck::{Arbitrary, Gen};
#[cfg(test)]
use std::convert::TryInto;

#[derive(Debug, Clone)]
/// Represents the top [TOP_LEVELS] `asks` and `bids` received from `exchange`.
///
/// `asks` and `bids` are assumed to be sorted in `release` and will panic in `debug` if this invariant is broken.
pub struct InputUpdate {
    exchange: Exchange,
    asks: ArrayVec<[Level; TOP_LEVELS]>,
    bids: ArrayVec<[Level; TOP_LEVELS]>,
}

impl InputUpdate {
    /// Returns a new [InputUpdate], `asks` and `bids` are assumed to be sorted in `release`
    /// and will panic in `debug` if this invariant is broken.
    pub fn new(
        exchange: Exchange,
        asks: ArrayVec<[Level; TOP_LEVELS]>,
        bids: ArrayVec<[Level; TOP_LEVELS]>,
    ) -> Self {
        debug_assert!(is_sorted(&asks, Level::cmp_ask), "Unsorted asks");
        debug_assert!(is_sorted(&bids, Level::cmp_bid), "Unsorted bids");

        Self {
            exchange,
            asks,
            bids,
        }
    }

    /// Consumes `self` and returns its contents.
    ///
    /// This approach was taken instead of public fields to be able to better
    /// maintain the sorting invariants in `ask` and `bids`.
    pub fn take(
        self,
    ) -> (
        Exchange,
        ArrayVec<[Level; TOP_LEVELS]>,
        ArrayVec<[Level; TOP_LEVELS]>,
    ) {
        let Self {
            exchange,
            asks,
            bids,
        } = self;
        (exchange, asks, bids)
    }
}

impl Into<orderbook::Summary> for InputUpdate {
    fn into(self) -> orderbook::Summary {
        let InputUpdate {
            asks,
            bids,
            exchange,
        } = self;

        let into_levels = |levels: ArrayVec<[Level; TOP_LEVELS]>| {
            levels
                .into_iter()
                .map(|level| level.into_orderbook_level(exchange))
        };

        let asks: Vec<orderbook::Level> = into_levels(asks).collect();

        let bids: Vec<orderbook::Level> = into_levels(bids).collect();

        orderbook::Summary {
            spread: asks[0].price - bids[0].price,
            asks,
            bids,
        }
    }
}

#[cfg(test)]
impl Arbitrary for InputUpdate {
    fn arbitrary(g: &mut Gen) -> Self {
        let exchange = (u8::arbitrary(g) % Exchange::VARIANT_COUNT as u8)
            .try_into()
            .unwrap();

        let arbitrary_levels = |g: &mut Gen| -> ArrayVec<[Level; TOP_LEVELS]> {
            let range = 0..(usize::arbitrary(g) % TOP_LEVELS);
            range.map(|_| Level::arbitrary(g)).collect()
        };
        let mut asks = arbitrary_levels(g);
        asks.sort_by(Level::cmp_ask);

        let mut bids = arbitrary_levels(g);
        bids.sort_by(Level::cmp_bid);

        Self {
            exchange,
            asks,
            bids,
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let exchange = self.exchange;
        let shrink_arrayvec =
            |arrayvec: &ArrayVec<_>| arrayvec.iter().cloned().collect::<Vec<_>>().shrink();

        Box::new(
            shrink_arrayvec(&self.asks)
                .zip(shrink_arrayvec(&self.bids))
                .map(move |(asks, bids)| Self {
                    exchange,
                    asks: asks.into_iter().collect(),
                    bids: bids.into_iter().collect(),
                }),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::arrayvec;
    use quickcheck_macros::quickcheck;

    #[test]
    fn test_into_summary() {
        assert_eq!(
            Into::<orderbook::Summary>::into(InputUpdate::new(
                Exchange::Binance,
                arrayvec![lvl!(1., 1.)],
                arrayvec![lvl!(0.5, 1.)]
            )),
            orderbook::Summary {
                asks: vec![lvl0!(1., 1.)],
                bids: vec![lvl0!(0.5, 1.)],
                spread: 0.5
            }
        );

        assert_eq!(
            Into::<orderbook::Summary>::into(InputUpdate::new(
                Exchange::Bitstamp,
                arrayvec![lvl!(1., 1.), lvl!(2., 1.)],
                arrayvec![lvl!(0.6, 1.), lvl!(0.3, 1.)]
            )),
            orderbook::Summary {
                asks: vec![lvl1!(1., 1.), lvl1!(2., 1.)],
                bids: vec![lvl1!(0.6, 1.), lvl1!(0.3, 1.)],
                spread: 0.4
            }
        );
    }

    #[test]
    #[should_panic(expected = "Unsorted asks")]
    fn test_unsorted_asks() {
        InputUpdate::new(
            Exchange::Binance,
            arrayvec![lvl!(1., 1.), lvl!(0.5, 1.)],
            arrayvec![],
        );
    }

    #[test]
    #[should_panic(expected = "Unsorted bids")]
    fn test_unsorted_bids() {
        InputUpdate::new(
            Exchange::Binance,
            arrayvec![],
            arrayvec![lvl!(0.5, 1.), lvl!(1., 1.)],
        );
    }

    #[quickcheck]
    fn test_arbitrary(inputs: Vec<InputUpdate>) {
        for input in inputs {
            assert!(is_sorted(&input.asks, Level::cmp_ask));
            assert!(is_sorted(&input.bids, Level::cmp_bid));
        }
    }
}
