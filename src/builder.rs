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

    partition_key_bits: u64,
    partition_key_mask: u64,

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

    fn build(mut self, false_positive_pow: usize) -> Result<Self::Filter, Self::Error> {
        self.false_positive_pow = false_positive_pow as u64;
        let f = self.build_it();
        Ok(f)
    }
}

impl Builder {
    pub fn new(false_positive_pow: u64) -> Self {
        Self {
            false_positive_pow,
            keys: Default::default(),
            param: Default::default(),
        }
    }

    /// 1. split into 64 words groups
    /// 1. find min suffix size
    /// 2. for every group: build suffix
    fn build_it(&mut self) -> SlimFilter {
        self.init_param();

        let segs = self.build_segments();

        self.init_suffix_param(&segs);
        let suffixes = self.build_suffixes(&segs);
        // println!("suffixes: {}", suffixes.words());

        self.init_partition_param(&segs);
        let partition_index = self.build_partition_keys(&segs);
        // println!("partition_keys: {}", partition_index.words());

        SlimFilter {
            word_bits: self.param.word_bits,
            partition_key_bits: self.param.partition_key_bits,
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

    fn build_partition_keys(&self, segs: &[Segment]) -> Bitmap {
        let mut partition_keys = Bitmap::new(
            self.param.partition_key_bits * segs.len() as u64,
            self.param.partition_key_bits,
        );

        if self.param.partition_key_bits > 0 {
            let mut prev = None;
            for seg in segs.iter() {
                let k = seg.keys[63] >> (64 - self.param.partition_key_bits);
                partition_keys.push_word(k);

                assert!(prev != Some(k));
                prev = Some(k);
            }
        }

        partition_keys
    }

    /// Use the max common prefix length+1 of every segment,
    /// guarantees that all these prefixes are ascending.
    fn init_partition_param(&mut self, segs: &[Segment]) {
        let mut pks = Vec::with_capacity(segs.len() * 2);
        for seg in segs.iter() {
            pks.push(seg.keys[0]);
            pks.push(seg.keys[63]);
        }

        let mut partition_key_bits = 0;
        for i in 0..pks.len() - 1 {
            let sig = (pks[i] ^ pks[i + 1]).leading_zeros();
            // Use the max common prefix length+1 of every segment,
            let pk_len = sig + 1;
            let pk_len = std::cmp::min(pk_len as u64, self.param.word_bits);
            partition_key_bits = std::cmp::max(partition_key_bits, pk_len);
        }

        // for seg in segs.iter() {
        //     let s = seg.big_suffix_bits();
        //     // println!("seg suffix bits: {}", s);
        //     let p = self.param.word_bits - s;
        //     partition_key_bits = std::cmp::max(partition_key_bits, p);
        // }

        partition_key_bits += 1;

        self.param.partition_key_bits = partition_key_bits;
        self.param.partition_key_mask = (1 << partition_key_bits) - 1;
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

            // uninitialized
            partition_key_bits: 0,
            partition_key_mask: 0,
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
    use std::collections::hash_map::DefaultHasher;
    use std::collections::BTreeSet;
    use std::hash::BuildHasher;
    use std::hash::BuildHasherDefault;
    use std::hash::Hash;
    use std::hash::Hasher;

    use crate::builder::Builder;
    use crate::traits::Filter;
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
        b.init_param();

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
        b.init_param();

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
        // 123456789012
        //  ----------- suffix

        for i in 1..67 {
            b.add_keys(&[i << shift]);
        }
        let p = b.init_param();
        assert_eq!(12, p.word_bits);

        let segs = b.build_segments();
        b.init_suffix_param(&segs);
        assert_eq!(11, b.param.suffix_bits);

        let suffixes = b.build_suffixes(&segs);
        println!("suffixes: {}", suffixes.words());

        assert_eq!(0b000_0001_0000, suffixes.get_word(0));
        assert_eq!(0b100_0000_0000, suffixes.get_word(63));
        assert_eq!(0b100_0010_0000, suffixes.get_word(65));
    }

    #[test]
    fn test_init_partition_param() {
        let word_bits = 8;
        let shift = 64 - word_bits;

        let mut b = Builder::new(5);

        for i in 1..67 {
            b.add_keys(&[i << shift]);
        }
        let p = b.init_param();
        assert_eq!(12, p.word_bits);

        let segs = b.build_segments();
        println!("segs[0]: suffix bits: {}", segs[0].big_suffix_bits());
        println!("segs[1]: suffix bits: {}", segs[1].big_suffix_bits());

        b.init_partition_param(&segs);
        assert_eq!(6, b.param.partition_key_bits);
    }

    #[test]
    fn test_build_partition_keys() {
        let word_bits = 8;
        let shift = 64 - word_bits;

        let mut b = Builder::new(5);

        for i in 1..67 {
            b.add_keys(&[i << shift]);
        }
        let p = b.init_param();
        assert_eq!(12, p.word_bits);

        let segs = b.build_segments();
        println!("segs[0]: suffix bits: {}", segs[0].big_suffix_bits());
        println!("segs[1]: suffix bits: {}", segs[1].big_suffix_bits());

        b.init_partition_param(&segs);
        assert_eq!(6, b.param.partition_key_bits);

        let pks = b.build_partition_keys(&segs);
        assert_eq!(2, pks.word_count);

        // 01000000
        // --------
        // 01234567----

        assert_eq!(0, pks.get_word(0));
        assert_eq!(0b010000, pks.get_word(1));
    }

    #[test]
    fn test_filter() {
        let x = BuildHasherDefault::<DefaultHasher>::default();

        let n = 1 << 20;
        let n = 100;

        let ks: BTreeSet<u64> = (0..n)
            .map(|i| {
                let hashed_key = {
                    let mut hasher = x.build_hasher();
                    i.hash(&mut hasher);
                    hasher.finish()
                };
                // println!("{:064b}", hashed_key);
                hashed_key
            })
            .collect();

        let keys = ks.iter().copied().collect::<Vec<_>>();

        let mut b = Builder::new(8);

        b.add_keys(&keys);
        let f = b.build(8).unwrap();
        println!("filter: {}", f.display(true));

        // 111110001001011
        // 101111111101100

        println!(": {:b}", 13823855910875200017u64);
        f.contains(&13823855910875200017u64);

        for k in keys.iter() {
            assert!(f.contains(k), "{} is in filter", k);
        }

        let mut hit = 0;
        let mut miss = 0;
        let ratio = 100;
        for i in n..n * ratio {
            let hashed_key = {
                let mut hasher = x.build_hasher();
                i.hash(&mut hasher);
                hasher.finish()
            };

            if ks.contains(&hashed_key) {
                continue;
            }

            if f.contains(&hashed_key) {
                hit += 1;
            } else {
                miss += 1;
            }
        }
        println!("hit: {}, miss: {}, 1/fp: {}", hit, miss, miss / (hit + 1))
    }
}
