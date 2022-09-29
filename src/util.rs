pub(crate) fn bit_count<T>(v: &T) -> u64 {
    (std::mem::size_of::<T>() * 8) as u64
}

pub(crate) fn next_multiple_of(v: u64, mul: u64) -> u64 {
    // TODO: test
    (v + mul - 1) / mul * mul
}

#[cfg(test)]
mod tests {
    use crate::util::next_multiple_of;

    #[test]
    fn test_next_multiple_of() {
        assert_eq!(0, next_multiple_of(0, 1));
        assert_eq!(0, next_multiple_of(0, 5));

        assert_eq!(1, next_multiple_of(1, 1));
        assert_eq!(5, next_multiple_of(1, 5));
        assert_eq!(10, next_multiple_of(6, 5));
    }
}
