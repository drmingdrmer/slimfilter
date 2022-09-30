use crate::traits::Key;

#[derive(Debug)]
pub(crate) struct Segment {
    pub(crate) word_bits: u64,
    pub(crate) keys: Vec<Key>,
}

impl Segment {
    pub(crate) fn new(word_bits: u64, keys: &[Key]) -> Self {
        debug_assert!(keys.len() > 0);

        // TODO: assert keys are sorted
        if keys.len() == 64 {
            Self {
                word_bits,
                keys: keys.to_vec(),
            }
        } else {
            // Fill with last key
            let mut ks = Vec::with_capacity(64);
            ks.extend_from_slice(keys);
            for _i in keys.len()..64 {
                ks.push(keys[keys.len() - 1]);
            }
            Self {
                word_bits,
                keys: ks,
            }
        }
    }

    /// Returns common prefix length and the content aligned to most significant bits.
    ///
    /// No two segments have the same common prefix.
    pub(crate) fn common_prefix(&self) -> (u64, u64) {
        let l = self.common_prefix_bits();
        (l, self.keys[0] & u64::MAX << (64 - l))
    }

    pub(crate) fn common_prefix_bits(&self) -> u64 {
        //
        let a = self.keys[0];
        let b = self.keys[63];
        let c = a ^ b;

        std::cmp::min(c.leading_zeros() as u64, self.word_bits)
    }

    pub(crate) fn big_suffix_bits(&self) -> u64 {
        self.word_bits - self.common_prefix_bits()
    }

    #[allow(dead_code)]
    pub(crate) fn suffix_bits(&self) -> u64 {
        // skip prefix and a 6-bit bitmap(size=64)
        self.word_bits - self.common_prefix_bits() - 6
    }
}

#[cfg(test)]
mod tests {
    use crate::segment::Segment;

    #[test]
    fn test_new() {
        let word_bits = 6;
        let shift = 64 - word_bits;
        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_0001 << shift,
            0b00_0011 << shift,
            0b00_0100 << shift,
            0b00_0111 << shift,
            0b01_0111 << shift,
            0b01_1001 << shift,
        ]);

        assert_eq!(word_bits, seg.word_bits);
        assert_eq!(
            vec![
                0b00_0000 << shift,
                0b00_0001 << shift,
                0b00_0011 << shift,
                0b00_0100 << shift,
                0b00_0111 << shift,
                0b01_0111 << shift,
                0b01_1001 << shift,
                0b01_1001 << shift,
            ],
            seg.keys[0..8]
        );
        assert_eq!(0b01_1001 << shift, seg.keys[63]);
    }

    #[test]
    fn test_common_prefix() {
        let word_bits = 6;
        let shift = 64 - word_bits;

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
        ]);
        assert_eq!((6, 0b00_0000 << shift), seg.common_prefix());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_0001 << shift,
        ]);
        assert_eq!((5, 0b00_0000 << shift), seg.common_prefix());

        let seg = Segment::new(word_bits, &[
            //
            0b00_1000 << shift,
            0b00_1001 << shift,
        ]);
        assert_eq!((5, 0b00_1000 << shift), seg.common_prefix());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_1001 << shift,
        ]);
        assert_eq!((2, 0b00_0000 << shift), seg.common_prefix());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b01_0111 << shift,
            0b01_1001 << shift,
        ]);
        assert_eq!((1, 0b00_0000 << shift), seg.common_prefix());

        let seg = Segment::new(word_bits, &[
            //
            0b10_0000 << shift,
            0b11_0111 << shift,
            0b11_1001 << shift,
        ]);
        assert_eq!((1, 0b10_0000 << shift), seg.common_prefix());
    }

    #[test]
    fn test_common_prefix_bits() {
        let word_bits = 6;
        let shift = 64 - word_bits;

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
        ]);
        assert_eq!(6, seg.common_prefix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_0001 << shift,
        ]);
        assert_eq!(5, seg.common_prefix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_1000 << shift,
            0b00_1001 << shift,
        ]);
        assert_eq!(5, seg.common_prefix_bits());
        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_1001 << shift,
        ]);
        assert_eq!(2, seg.common_prefix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_0001 << shift,
            0b00_0011 << shift,
            0b00_0100 << shift,
            0b00_0111 << shift,
            0b01_0111 << shift,
            0b01_1001 << shift,
        ]);

        assert_eq!(1, seg.common_prefix_bits());
    }

    #[test]
    fn test_big_suffix_bits() {
        let word_bits = 6;
        let shift = 64 - word_bits;

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
        ]);
        assert_eq!(0, seg.big_suffix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_0001 << shift,
        ]);
        assert_eq!(1, seg.big_suffix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_1000 << shift,
            0b00_1001 << shift,
        ]);
        assert_eq!(1, seg.big_suffix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_1001 << shift,
        ]);
        assert_eq!(4, seg.big_suffix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b00_0001 << shift,
            0b00_0011 << shift,
            0b00_0100 << shift,
            0b00_0111 << shift,
            0b01_0111 << shift,
            0b01_1001 << shift,
        ]);
        assert_eq!(5, seg.big_suffix_bits());

        let seg = Segment::new(word_bits, &[
            //
            0b00_0000 << shift,
            0b01_0111 << shift,
            0b10_1001 << shift,
        ]);

        assert_eq!(6, seg.big_suffix_bits());
    }
}
