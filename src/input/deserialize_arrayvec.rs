use arrayvec::ArrayVec;
use serde::{
    de::{IgnoredAny, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use std::fmt;
use std::marker::PhantomData;

/// Wrapper around [ArrayVec] to deserialize only the first [ArrayVec::capacity] items and ignore the rest.
pub struct DeserializeArrayVec<T: arrayvec::Array>(ArrayVec<T>);

impl<T: arrayvec::Array> Into<ArrayVec<T>> for DeserializeArrayVec<T> {
    fn into(self) -> ArrayVec<T> {
        self.0
    }
}

impl<'de, Item: Deserialize<'de>, T: arrayvec::Array<Item = Item>> Deserialize<'de>
    for DeserializeArrayVec<T>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SeqVisitor<T>(PhantomData<T>);
        impl<'de, Item: Deserialize<'de>, T: arrayvec::Array<Item = Item>> Visitor<'de> for SeqVisitor<T> {
            type Value = DeserializeArrayVec<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("DeserializeArrayVec")
            }
            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let mut elems = ArrayVec::<T>::new();
                while let (false, Some(elem)) = (elems.is_full(), seq.next_element()?) {
                    elems.push(elem)
                }
                // All items must be consumed but we only care about the first n ones.
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(DeserializeArrayVec(elems))
            }
        }
        deserializer.deserialize_seq(SeqVisitor(PhantomData))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::arrayvec;

    #[test]
    fn test_deserialize() {
        let deserialize = |s: &str| -> Option<ArrayVec<[f64; 3]>> {
            simd_json::from_str::<DeserializeArrayVec<[f64; 3]>>(&mut s.to_string())
                .ok()
                .map(Into::into)
        };
        assert_eq!(deserialize("[1,2,3]").unwrap(), arrayvec![1., 2., 3.]);

        assert_eq!(deserialize("[1]").unwrap(), arrayvec![1.]);

        assert_eq!(deserialize("[]").unwrap(), arrayvec![]);

        assert_eq!(deserialize("[1,2,3,4]").unwrap(), arrayvec![1., 2., 3.]);

        assert!(deserialize("").is_none());
        assert!(deserialize("sfsda").is_none());
        assert!(deserialize("[[]]").is_none());
    }
}
