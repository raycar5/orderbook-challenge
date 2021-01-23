#[macro_use]
mod level;
pub mod sources;
pub use level::*;
mod finite_positive_f64;
pub use finite_positive_f64::*;
mod input_update;
pub use input_update::*;
mod deserialize_arrayvec;
pub use deserialize_arrayvec::*;

#[cfg(test)]
#[macro_export]
macro_rules! arrayvec {
    ($($input:tt)*) => {
        {
            let a: ::arrayvec::ArrayVec<_>= vec![$($input)*].into_iter().collect();
            a
        }
    };
}
