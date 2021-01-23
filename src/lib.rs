use std::cmp::Ordering;

#[macro_use]
pub mod input;
pub mod merge;
pub mod proto;
pub mod serve;

/// Number of items in the channel between the parsers and the merger.
pub const CHANNEL_SIZE: usize = 100;
/// Number of top levels to display per [Exchange](input::Exchange).
pub const TOP_LEVELS: usize = 10;

pub fn is_sorted<T>(levels: &[T], cmp_fn: impl Fn(&T, &T) -> Ordering) -> bool {
    levels
        .windows(2)
        .all(|w| !matches!(cmp_fn(&w[0], &w[1]), Ordering::Greater))
}

#[cfg(test)]
mod test {
    use input::Level;

    use super::*;
    #[test]
    fn test_is_sorted() {
        assert!(is_sorted(&[], Level::cmp_ask));
        assert!(is_sorted(&[], Level::cmp_bid));

        assert!(is_sorted(&[lvl!(1., 1.)], Level::cmp_ask));
        assert!(is_sorted(&[lvl!(1., 1.)], Level::cmp_bid));

        assert!(is_sorted(&[lvl!(1., 1.), lvl!(1., 1.)], Level::cmp_ask));
        assert!(is_sorted(&[lvl!(1., 1.), lvl!(1., 1.)], Level::cmp_bid));

        assert!(is_sorted(&[lvl!(1., 1.), lvl!(2., 1.)], Level::cmp_ask));
        assert!(is_sorted(&[lvl!(2., 1.), lvl!(1., 1.)], Level::cmp_bid));

        assert!(!is_sorted(&[lvl!(2., 1.), lvl!(1., 1.)], Level::cmp_ask));
        assert!(!is_sorted(&[lvl!(1., 1.), lvl!(2., 1.)], Level::cmp_bid));

        assert!(is_sorted(&[lvl!(1., 2.), lvl!(1., 1.)], Level::cmp_ask));
        assert!(is_sorted(&[lvl!(1., 2.), lvl!(1., 1.)], Level::cmp_bid));

        assert!(!is_sorted(&[lvl!(1., 1.), lvl!(1., 2.)], Level::cmp_ask));
        assert!(!is_sorted(&[lvl!(1., 1.), lvl!(1., 2.)], Level::cmp_bid));
    }
}
