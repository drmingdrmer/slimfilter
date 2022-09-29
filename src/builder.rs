use std::collections::BTreeSet;

use crate::bitmap::Bitmap;
use crate::filter::SlimFilter;
use crate::segment::Segment;
use crate::traits::FilterBuilder;
use crate::traits::Key;
use crate::util::next_multiple_of;

#[derive(Default)]
struct BuildingParam {
    cardinality: u64,

    /// ceil(log2(cardinality))
    cardinality_pow: u64,

    pref_bits: u64,
    suffix_bits: u64,
    suffix_mask: u64,

    /// pref_bits + suffix_bits
    word_bits: u64,
}

#[derive(Default)]
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

        self.init_param();

        let segs = self.build_segments();

        self.init_suffix_param(&segs);
        let suffixes = self.build_suffixes(&segs);

        // Find partition key bits

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
        let mut partition_index =
            Bitmap::new(partition_key_bits * segs.len() as u64, partition_key_bits);
        for (i, pk) in partition_keys.iter().enumerate() {
            partition_index.push_word(pk >> (64 - partition_key_bits));
        }

        SlimFilter {
            partition_key_bits,
            partitions: partition_index,
            suffix_bits: self.param.suffix_bits,
            suffixes,
        }
    }

    fn build_segments(&self) -> Vec<Segment> {
        let p = &self.param;

        let mut segs = Vec::with_capacity(next_multiple_of(p.cardinality, 64) as usize / 64);

        let mut ks = [0; 64];
        let mut count = 0;
        for k in self.keys.iter() {
            ks[count] = *k;
            count += 1;
            // println!("count: {:3}, key: {:64b}", count, *k);
            if count == 64 {
                let seg = Segment::new(p.word_bits, &ks);
                segs.push(seg);
                count = 0;
            }
        }

        // last segment: fill padding keys
        if count > 0 {
            for i in count..64 {
                ks[i] = ks[count - 1];
            }
            let seg = Segment::new(p.word_bits, &ks);
            segs.push(seg);
        }
        segs
    }

    fn init_suffix_param(&mut self, segs: &[Segment]) {
        let mut suffix_bits = 0;
        for seg in segs.iter() {
            let s = seg.big_suffix_bits();
            suffix_bits = std::cmp::max(suffix_bits, s);
        }

        self.param.suffix_bits = suffix_bits;
        self.param.suffix_mask = (1 << suffix_bits) - 1;
    }

    fn build_suffixes(&mut self, segs: &[Segment]) -> Bitmap {
        //
        let mut suffixes = Bitmap::new(
            self.param.suffix_bits * segs.len() as u64 * 64,
            self.param.suffix_bits,
        );
        for seg in segs.iter() {
            for j in 0..64 {
                let suffix = (seg.keys[j] >> (64 - self.param.word_bits)) & self.param.suffix_mask;
                suffixes.push_word(suffix);
            }
        }

        suffixes
    }

    fn init_param(&mut self) -> &BuildingParam {
        let n = self.keys.len() as u64;
        let n_pow = self.n_next_pow().trailing_zeros() as u64;
        // 2^(-fp) >= n/2^word_bits
        let word_bits = n_pow + self.false_positive_pow;

        self.param = BuildingParam {
            cardinality: n,
            cardinality_pow: n_pow,
            word_bits,

            // TODO
            pref_bits: 0,
            suffix_bits: 0,
            suffix_mask: 0,
        };
        &self.param
    }

    /// Prefix bitmap size is the
    fn n_next_pow(&self) -> u64 {
        let n = self.keys.len();
        let prefix_bitmap_bit = n.next_power_of_two();

        prefix_bitmap_bit as u64
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::Builder;
    use crate::traits::FilterBuilder;
    use crate::traits::Key;

    #[test]
    fn test_new() {
        let b = Builder::new(5);
        assert_eq!(5, b.false_positive_pow);
    }

    #[test]
    fn test_add_keys() {
        let mut b = Builder::new(5);
        assert_eq!(5, b.false_positive_pow);

        b.add_keys(&[1, 4, 5, 5, 2, 3]);
        assert_eq!(
            vec![1, 2, 3, 4, 5],
            b.keys.iter().copied().collect::<Vec<Key>>()
        );
    }

    #[test]
    fn test_init_param() {
        let mut b = Builder::new(5);
        assert_eq!(5, b.false_positive_pow);

        b.add_keys(&[1, 4, 5, 6, 2, 6, 3]);
        let p = b.init_param();

        assert_eq!(6, p.cardinality);
        assert_eq!(3, p.cardinality_pow);
        assert_eq!(8, p.word_bits);
    }

    #[test]
    fn test_build_segments_1() {
        let word_bits = 6;
        let shift = 64 - word_bits;

        let mut b = Builder::new(5);

        b.add_keys(&[
            //
            0b00_0000 << shift,
            0b00_0001 << shift,
            0b00_0010 << shift,
            0b01_0100 << shift,
            0b01_1000 << shift,
        ]);
        let p = b.init_param();

        let segs = b.build_segments();
        assert_eq!(1, segs.len());
        assert_eq!(0b00_0000 << shift, segs[0].keys[0]);
        assert_eq!(0b00_0001 << shift, segs[0].keys[1]);
        assert_eq!(0b01_1000 << shift, segs[0].keys[63]);
    }

    #[test]
    fn test_build_segments_2() {
        let word_bits = 8;
        let shift = 64 - word_bits;

        let mut b = Builder::new(5);

        for i in 1..67 {
            b.add_keys(&[i << shift]);
        }
        let p = b.init_param();

        let segs = b.build_segments();

        assert_eq!(2, segs.len());
        assert_eq!(0b0000_0001 << shift, segs[0].keys[0]);
        assert_eq!(0b0000_0010 << shift, segs[0].keys[1]);
        assert_eq!(0b0100_0000 << shift, segs[0].keys[63]);

        assert_eq!(0b0100_0001 << shift, segs[1].keys[0]);
        assert_eq!(0b0100_0010 << shift, segs[1].keys[63]);
    }

    #[test]
    fn test_init_suffix_param() {
        let word_bits = 8;
        let shift = 64 - word_bits;

        let mut b = Builder::new(5);

        for i in 1..67 {
            b.add_keys(&[i << shift]);
        }
        let p = b.init_param();
        assert_eq!(12, p.word_bits);

        let segs = b.build_segments();

        b.init_suffix_param(&segs);
        assert_eq!(11, b.param.suffix_bits);
        assert_eq!(0b0111_1111_1111, b.param.suffix_mask);
    }

    #[test]
    fn test_build_suffixes() {
        let word_bits = 8;
        let shift = 64 - word_bits;

        let mut b = Builder::new(5);

        for i in 1..67 {
            b.add_keys(&[i << shift]);
        }
        let p = b.init_param();
        assert_eq!(12, p.word_bits);

        let segs = b.build_segments();
        b.init_suffix_param(&segs);
        assert_eq!(11, b.param.suffix_bits);

        let suffixes = b.build_suffixes(&segs);

        assert_eq!(0b000_0001_0000, suffixes.get_word(0));
    }
}
