use parse_display::Display;
#[cfg(test)]
use quickcheck::{Arbitrary, Gen};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};
use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt,
};

#[derive(Debug, Clone, Copy, Display, PartialEq, PartialOrd)]
/// Contains an f64 which [is positive](f64::is_sign_positive) and [finite](f64::is_finite).
pub struct FinitePositiveF64(f64);

impl Into<f64> for FinitePositiveF64 {
    fn into(self) -> f64 {
        self.0
    }
}

impl Eq for FinitePositiveF64 {}

#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for FinitePositiveF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other)
            .expect("FinitePositiveF64 invariants violated")
    }
}

impl TryFrom<f64> for FinitePositiveF64 {
    type Error = &'static str;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            return Err("Can't construct FinitePositiveF64 from non finite f64");
        }
        if !value.is_sign_positive() {
            return Err("Can't construct FinitePositiveF64 from negative f64");
        }
        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for FinitePositiveF64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrVisitor;
        impl<'de> Visitor<'de> for StrVisitor {
            type Value = FinitePositiveF64;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("FinitePositiveF64")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                fast_float::parse(value)
                    .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(value), &self))
                    .and_then(|value: f64| {
                        value.try_into().map_err(|_| {
                            de::Error::invalid_value(de::Unexpected::Float(value), &self)
                        })
                    })
            }
        }
        deserializer.deserialize_str(StrVisitor)
    }
}

#[cfg(test)]
/// Returns `n.abs()` or 0 if `n` is not finite.
fn clean_f64(n: f64) -> f64 {
    if n.is_finite() {
        n.abs()
    } else {
        0.
    }
}

#[cfg(test)]
impl Arbitrary for FinitePositiveF64 {
    fn arbitrary(g: &mut Gen) -> Self {
        Self(clean_f64(f64::arbitrary(g)))
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(self.0.shrink().map(|f| Self(clean_f64(f))))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use quickcheck_macros::quickcheck;
    #[test]
    fn test_try_from() {
        assert_eq!(3.0.try_into(), Ok(FinitePositiveF64(3.)));
        assert_eq!(0.0.try_into(), Ok(FinitePositiveF64(0.)));

        assert_eq!(
            TryInto::<FinitePositiveF64>::try_into(-3.0),
            Err("Can't construct FinitePositiveF64 from negative f64")
        );
        assert_eq!(
            TryInto::<FinitePositiveF64>::try_into(-0.0),
            Err("Can't construct FinitePositiveF64 from negative f64")
        );

        assert_eq!(
            TryInto::<FinitePositiveF64>::try_into(std::f64::NAN),
            Err("Can't construct FinitePositiveF64 from non finite f64")
        );
        assert_eq!(
            TryInto::<FinitePositiveF64>::try_into(std::f64::INFINITY),
            Err("Can't construct FinitePositiveF64 from non finite f64")
        );
        assert_eq!(
            TryInto::<FinitePositiveF64>::try_into(std::f64::NEG_INFINITY),
            Err("Can't construct FinitePositiveF64 from non finite f64")
        );
    }

    #[test]
    fn test_deserialize() {
        assert_eq!(
            simd_json::from_str(&mut r#""0""#.to_string()),
            Ok(FinitePositiveF64(0.))
        );
        assert_eq!(
            simd_json::from_str(&mut r#""1.4""#.to_string()),
            Ok(FinitePositiveF64(1.4))
        );

        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#""""#.to_string()).is_err(),);
        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#""#.to_string()).is_err(),);
        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#"0"#.to_string()).is_err(),);
        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#""-0""#.to_string()).is_err(),);
        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#""-3.4""#.to_string()).is_err(),);
        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#""blah""#.to_string()).is_err(),);
        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#""1e500""#.to_string()).is_err(),);
        assert!(simd_json::from_str::<FinitePositiveF64>(&mut r#""  1.4  ""#.to_string()).is_err(),);
    }

    #[test]
    fn test_clean() {
        assert_eq!(clean_f64(0.), 0.);
        assert_eq!(clean_f64(1.6), 1.6);

        assert_eq!(clean_f64(-5.), 5.);

        assert_eq!(clean_f64(std::f64::NAN), 0.);
        assert_eq!(clean_f64(std::f64::INFINITY), 0.);
        assert_eq!(clean_f64(std::f64::NEG_INFINITY), 0.);
    }

    #[quickcheck]
    fn test_arbitrary(floats: Vec<FinitePositiveF64>) {
        for float in floats {
            assert!(float.0.is_finite());
            assert!(float.0.is_sign_positive());
        }
    }
}
