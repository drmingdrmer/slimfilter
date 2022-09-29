use std::collections::BTreeSet;

use crate::bitmap::Bitmap;
use crate::filter::SlimFilter;
use crate::segment::Segment;
use crate::traits::FilterBuilder;
use crate::traits::Key;
use crate::util::next_multiple_of;

#[derive(Default)]
struct BuildingParam {
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
    fn build_it(&mut self) -> SlimFilter {
        //

        // self.param = BuildingParam {
        //     pref_bits,
        //     suffix_bits,
        //     suffix_mask: (1 << suffix_bits) - 1,
        //     word_bits,
        // };

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

        let mut suffixes = Bitmap::new(max_suffix_bits * n_pow, max_suffix_bits);
        for (i, seg) in segs.iter().enumerate() {
            for j in 0..64 {
                let suffix = (seg.keys[j] >> word_bits) & ((1 << max_suffix_bits) - 1);
                suffixes.push_word(suffix);
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
        let mut partition_index = Bitmap::new(partition_key_bits * segs.len() as u64, partition_key_bits);
        for (i, pk) in partition_keys.iter().enumerate() {
            partition_index.push_word(pk >> (64 - partition_key_bits));
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
        let n = self.keys.len() as u64;
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

    /// Prefix bitmap size is the
    fn n_next_pow(&self) -> u64 {
        let n = self.keys.len();
        let prefix_bitmap_bit = n.next_power_of_two();

        prefix_bitmap_bit as u64
    }
}
