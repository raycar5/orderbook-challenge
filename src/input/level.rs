use super::FinitePositiveF64;
use crate::proto::orderbook;
use num_enum::TryFromPrimitive;
use parse_display::Display;
#[cfg(test)]
use quickcheck::{Arbitrary, Gen};
use serde::Deserialize;
use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
};
use variant_count::VariantCount;

#[derive(Display, PartialEq, Debug, VariantCount, Clone, Copy, TryFromPrimitive)]
#[display(style = "lowercase")]
#[repr(u8)]
/// Represents the source exchange for a particular price level.
pub enum Exchange {
    Binance = 0,
    Bitstamp = 1,
}

#[derive(Deserialize, PartialEq, Clone, Copy, Debug)]
/// Represents a price level in an exchange.
pub struct Level {
    pub price: FinitePositiveF64,
    pub amount: FinitePositiveF64,
}

impl Level {
    /// Returns a new [orderbook::Level] with the provided `exchange`.
    pub fn into_orderbook_level(self, exchange: Exchange) -> orderbook::Level {
        let Level { price, amount } = self;
        orderbook::Level {
            price: price.into(),
            amount: amount.into(),
            exchange: exchange.to_string(),
        }
    }

    /// Orders [Levels](Level) such that
    /// ```{ price: 2, amount: 1 } < { price: 1, amount: 1 }```
    /// and
    /// ```{ price: 1, amount: 2 } < { price: 1, amount: 1 }```.
    pub fn cmp_bid(&self, other: &Self) -> Ordering {
        let c = self.price.cmp(&other.price);
        if c == Ordering::Equal {
            self.amount.cmp(&other.amount).reverse()
        } else {
            c.reverse()
        }
    }

    /// Orders [Levels](Level) such that
    /// ```{ price: 1, amount: 1 } < { price: 2, amount: 1 }```
    /// and
    /// ```{ price: 1, amount: 2 } < { price: 1, amount: 1 }```.
    pub fn cmp_ask(&self, other: &Self) -> Ordering {
        let c = self.price.cmp(&other.price);
        if c == Ordering::Equal {
            self.amount.cmp(&other.amount).reverse()
        } else {
            c
        }
    }
}

impl TryFrom<&orderbook::Level> for Level {
    type Error = &'static str;
    fn try_from(other: &orderbook::Level) -> Result<Self, Self::Error> {
        let orderbook::Level { price, amount, .. } = other;
        Ok(Self {
            price: (*price).try_into()?,
            amount: (*amount).try_into()?,
        })
    }
}

#[cfg(test)]
impl Arbitrary for Level {
    fn arbitrary(g: &mut Gen) -> Self {
        Self {
            price: FinitePositiveF64::arbitrary(g),
            amount: FinitePositiveF64::arbitrary(g),
        }
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(
            self.price
                .shrink()
                .zip(self.amount.shrink())
                .map(|(price, amount)| Self { price, amount }),
        )
    }
}

#[cfg(test)]
macro_rules! lvl {
    ($price:expr, $amount:expr) => {{
        use std::convert::TryInto;
        crate::input::Level {
            price: ($price).try_into().unwrap(),
            amount: ($amount).try_into().unwrap(),
        }
    }};
}
#[cfg(test)]
macro_rules! lvl0 {
    ($price:expr, $amount:expr) => {
        crate::proto::orderbook::Level {
            exchange: Exchange::Binance.to_string(),
            price: $price,
            amount: $amount,
        }
    };
}
#[cfg(test)]
macro_rules! lvl1 {
    ($price:expr, $amount:expr) => {
        crate::proto::orderbook::Level {
            exchange: Exchange::Bitstamp.to_string(),
            price: $price,
            amount: $amount,
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_into_orderbook_level() {
        assert_eq!(
            lvl!(1., 3.).into_orderbook_level(Exchange::Binance),
            lvl0!(1., 3.)
        );
        assert_eq!(
            lvl!(0., 5.).into_orderbook_level(Exchange::Bitstamp),
            lvl1!(0., 5.)
        );
    }

    #[test]
    fn test_cmp_bid() {
        assert_eq!(lvl!(1., 3.).cmp_bid(&lvl!(0.5, 5.)), Ordering::Less);

        assert_eq!(lvl!(1., 3.).cmp_bid(&lvl!(1., 3.)), Ordering::Equal);

        assert_eq!(lvl!(1., 3.).cmp_bid(&lvl!(1., 5.)), Ordering::Greater);
    }

    #[test]
    fn test_cmp_ask() {
        assert_eq!(lvl!(1., 3.).cmp_ask(&lvl!(0.5, 5.)), Ordering::Greater);

        assert_eq!(lvl!(1., 3.).cmp_ask(&lvl!(1., 3.)), Ordering::Equal);

        assert_eq!(lvl!(1., 3.).cmp_ask(&lvl!(1., 5.)), Ordering::Greater);
    }

    #[test]
    fn test_try_from() {
        assert_eq!((&lvl0!(1., 4.)).try_into(), Ok(lvl!(1., 4.)));

        assert!(TryInto::<Level>::try_into(&lvl0!(-1., 4.)).is_err());
        assert!(TryInto::<Level>::try_into(&lvl0!(1., -4.)).is_err());
        assert!(TryInto::<Level>::try_into(&lvl0!(1., std::f64::NAN)).is_err());
    }
}
