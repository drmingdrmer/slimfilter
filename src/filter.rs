use crate::bitmap::Bitmap;
use crate::traits::Filter;
use crate::traits::Key;

pub struct SlimFilter {
    pub(crate) partition_key_bits: u64,
    pub(crate) partitions: Bitmap,
    pub(crate) suffix_bits: u64,
    pub(crate) suffixes: Bitmap,
}

impl Filter for SlimFilter {
    fn contains(&self, key: &Key) -> bool {
        todo!()
    }
}
