pub(crate) mod bitmap;
pub(crate) mod builder;
pub(crate) mod filter;
pub(crate) mod traits;

#[cfg(test)] mod tests;

pub(crate) fn bit_count<T>(v: &T) -> u64 {
    (std::mem::size_of::<T>() * 8) as u64
}

pub(crate) fn next_multiple_of(v: u64, mul: u64) -> u64 {
    // TODO: test
    (v + mul - 1) % mul * mul
}
