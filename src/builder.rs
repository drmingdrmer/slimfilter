use std::collections::BTreeSet;
use std::fmt::rt::v1::Count::Param;
use std::simd::simd_swizzle;
use std::slice::from_ptr_range;

use crate::bit_count;
use crate::bitmap::Bitmap;
use crate::filter::SlimFilter;
use crate::next_multiple_of;
use crate::traits::FilterBuilder;
use crate::traits::Key;

#[derive(Default)]
struct BuildingParam {
    prefix_bitmap_bits: u64,
    pref_bits: u64,
    suffix_bits: u64,

    suffix_mask: u64,

    /// pref_bits + suffix_bits
    word_bits: u64,
}

pub struct Builder {
    false_positive_pow: u64,
    keys: BTreeSet<Key>,

    param: BuildingParam,
}

impl FilterBuilder for Builder {
    type Filter = SlimFilter;
    type Error = ();

    fn add_keys(&mut self, keys: &[Key]) {
        for key in keys {
            self.keys.insert(*key);
        }
    }

    fn build(self, false_positive_pow: usize) -> Result<Self::Filter, Self::Error> {
        todo!()
    }
}

impl Builder {
    pub fn new(false_positive_pow: usize) -> Self {
        Self {
            false_positive_pow: false_positive_pow as u64,
            keys: Default::default(),
            param: Default::default(),
        }
    }

    /// 1. split into 64 words groups
    /// 1. find min suffix size
    /// 2. for every group: build suffix
    fn build_it(&mut self) -> Self::Filter {
        //

        self.param = BuildingParam {
            prefix_bitmap_bits: bm_size,
            pref_bits,
            suffix_bits,
            suffix_mask: (1 << suffix_bits) - 1,
            word_bits,
        };

        let n = self.keys.len() as u64;

        let n_pow = self.n_next_pow().trailing_zeros() as u64;
        // 2^(-fp) >= n/2^word_bits
        let word_bits = n_pow + self.false_positive_pow + n_pow;

        let segs = self.build_segments();

        // find max suffix bits

        let mut max_suffix_bits = 0;
        for seg in segs.iter() {
            let suffix_bits = seg.big_suffix_bits();
            max_suffix_bits = std::cmp::max(max_suffix_bits, suffix_bits);
        }

        // build suffix

        let mut suffixes = Bitmap::new(max_suffix_bits * n_pow);
        for (i, seg) in segs.iter().enumerate() {
            for j in 0..64 {
                let suffix = (seg.keys[j] >> word_bits) & ((1 << max_suffix_bits) - 1);
                suffixes.push_word(i as u64 * 64 + j as u64, max_suffix_bits, suffix);
            }
        }

        // build partition keys:

        let mut partition_keys = Vec::with_capacity(segs.len());
        for seg in segs.iter() {
            partition_keys.push(seg.keys[63]);
        }

        // find smallest partition key bits

        let mut partition_key_bits = 0;
        let mut it = partition_keys.iter();
        let first = it.next().unwrap();
        for pk in it {
            let x = first ^ (*pk);
            let key_bits = x.leading_zeros() as u64 + 1;
            partition_key_bits = std::cmp::max(partition_key_bits, key_bits);
        }

        // bulid partition index:
        let mut partition_index = Bitmap::new(partition_key_bits * segs.len());
        for (i, pk) in partition_keys.iter().enumerate() {
            partition_index.push_word(i as u64, partition_key_bits, pk >> (64 - partition_key_bits));
        }

        SlimFilter {
            partition_key_bits,
            partitions: partition_index,
            suffix_bits: max_suffix_bits,
            suffixes,
        }
    }

    fn build_segments(&self) -> Vec<Segment> {
        let word_bits = self.n_next_pow().trailing_zeros() as u64;
        let mut segs = Vec::with_capacity(next_multiple_of(n, 64) as usize / 64);

        let mut ks = [0; 64];
        let mut count = 0;
        for k in self.keys.iter() {
            ks[count] = *k;
            count += 1;
            if count == 64 {
                let seg = Segment::new(word_bits, &ks);
                segs.push(seg);
                count = 0;
            }
        }

        // last segment: fill padding keys
        if count > 0 {
            for i in count..64 {
                ks[i] = ks[count - 1];
            }
            let seg = Segment::new(word_bits, &ks);
            segs.push(seg);
        }
        segs
    }

    fn build_prefix_bitmap(&self) -> Vec<u64> {
        assert!(self.keys.len() > 0);

        let bm_size = self.param.prefix_bitmap_bits;

        let pref_len = bm_size.trailing_zeros();

        let word_count = bm_size / u64::BITS;
        let mut bitmap = vec![0; word_count as usize];
        for key in self.keys.iter() {
            let pref = key >> (u64::BITS - pref_len);
            bitmap[pref >> 6] |= 1 << (pref & 0xff);
        }

        bitmap
    }

    fn get_prefix(&self, key: u64) -> u64 {}

    /// Returns an empty bitmap of at least `n` bits,
    /// and bitmap of `n` k-bit words.
    /// where `n = keys.len()`, `k` is suffix size
    fn build_suffix_array(&self) -> (Bitmap, Bitmap) {
        let n = self.keys.len() as u64;
        let flag_bm_bits = next_multiple_of(n, 64);

        let suffix_bits = self.false_positive_pow;

        let flag_bm = Bitmap::new(flag_bm_bits);
        let mut suffix_array = Bitmap::new(n * suffix_bits);

        for (i, key) in self.keys.iter().enumerate() {
            let word = key >> u64::BITS - self.param.word_bits;
            let suffix = word & self.param.suffix_mask;
            suffix_array.push_word(i as u64, self.param.suffix_bits, suffix);
        }

        (flag_bm, suffix_array)
    }

    /// Prefix bitmap size is the
    fn n_next_pow(&self) -> u64 {
        let n = self.keys.len();
        let prefix_bitmap_bit = n.next_power_of_two();

        prefix_bitmap_bit as u64
    }
}

pub(crate) struct Segment {
    word_bits: u64,
    keys: Vec<Key>,
}

impl Segment {
    pub(crate) fn new(word_bits: u64, keys: &[Key]) -> Self {
        // TODO: assert keys are sorted
        Self {
            word_bits,
            keys: keys.to_vec(),
        }
    }

    pub(crate) fn common_prefix_bits(&self) -> u64 {
        //
        let a = self.keys[0];
        let b = self.keys[63];
        let c = a ^ b;
        c.leading_zeros() as u64
    }
    pub(crate) fn big_suffix_bits(&self) -> u64 {
        self.word_bits - self.common_prefix_bits()
    }

    pub(crate) fn suffix_bits(&self) -> u64 {
        // skip prefix and a 6-bit bitmap(size=64)
        self.word_bits - self.common_prefix_bits() - 6
    }
}
