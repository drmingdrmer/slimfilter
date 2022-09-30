use std::fmt::Display;
use std::fmt::Formatter;

use crate::bitmap::Bitmap;
use crate::traits::Filter;
use crate::traits::Key;

pub struct SlimFilter {
    pub(crate) word_bits: u64,
    pub(crate) partition_key_bits: u64,
    pub(crate) partitions: Bitmap,
    pub(crate) suffix_bits: u64,
    pub(crate) suffixes: Bitmap,
}

pub struct DisplaySlimFilter<'a> {
    with_bitmap: bool,
    inner: &'a SlimFilter,
}

impl<'a> Display for DisplaySlimFilter<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "word_bits: {}, partition_key_bits: {}, suffix_bits: {}\n",
            self.inner.word_bits, self.inner.partition_key_bits, self.inner.suffix_bits
        )?;

        if self.with_bitmap {
            write!(f, "partitions:\n")?;
            Display::fmt(&self.inner.partitions.words(), f)?;

            write!(f, "suffixes:\n")?;
            Display::fmt(&self.inner.suffixes.words(), f)?;
        }
        Ok(())
    }
}

impl SlimFilter {
    pub fn display(&self, with_bitmap: bool) -> DisplaySlimFilter {
        DisplaySlimFilter {
            inner: &self,
            with_bitmap,
        }
    }
}

impl Filter for SlimFilter {
    fn contains(&self, key: &Key) -> bool {
        let idx;
        if self.partition_key_bits > 0 {
            let pref = key >> (64 - self.partition_key_bits);

            idx = self.partitions.find(pref);
            if idx >= self.partitions.word_count {
                return false;
            }
        } else {
            // there is only one part
            idx = 0;
        }

        // find in a partition

        let suffix = key >> (64 - self.word_bits);
        let suffix = suffix & ((1 << self.suffix_bits) - 1);

        let at = self.suffixes.find_range(suffix, idx as isize * 64, idx as isize * 64 + 64);
        if at >= self.suffixes.word_count {
            return false;
        }
        return self.suffixes.get_word(at) == suffix;
    }
}
