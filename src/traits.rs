pub type Key = u64;

pub trait Filter {
    fn contains(&self, key: &Key) -> bool;
}

pub trait FilterBuilder {
    type Filter: Filter;
    // type Error: std::error::Error;
    type Error;

    fn add_keys(&mut self, keys: &[Key]);

    /// The power of false_positive.
    ///
    /// E.g. false_positive_pow = 8 means fp = 1/2^8
    fn build(self, false_positive_pow: usize) -> Result<Self::Filter, Self::Error>;
}
