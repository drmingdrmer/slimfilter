use std::fmt::Display;
use std::fmt::Formatter;

use crate::util::next_multiple_of;

pub(crate) struct Bitmap {
    pub(crate) word_bits: u64,
    pub(crate) word_count: u64,
    pub(crate) bm: Vec<u64>,
}

pub(crate) struct DisplayBitmap<'a> {
    bm: &'a Bitmap,
}

impl<'a> Display for DisplayBitmap<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "word_bits: {}, word_count: {}\n",
            self.bm.word_bits, self.bm.word_count
        )?;

        for i in 0..self.bm.word_count {
            write!(
                f,
                "{:05}: {:0width$b}\n",
                i,
                self.bm.get_word(i),
                width = self.bm.word_bits as usize
            )?;
        }

        Ok(())
    }
}

impl Bitmap {
    /// Create a bitmap with at least n bits
    pub(crate) fn new(n: u64, word_bits: u64) -> Self {
        Self {
            word_bits,
            word_count: 0,
            bm: vec![0; next_multiple_of(n, 64) as usize / 64],
        }
    }

    pub(crate) fn words(&self) -> DisplayBitmap {
        //
        DisplayBitmap { bm: &self }
    }

    pub(crate) fn set(&mut self, i: u64) {
        let word_index = i >> 6;
        let bit_index = i & 63;
        self.bm[word_index as usize] |= 1 << bit_index;
    }

    // word is aligned to least significant bits
    pub(crate) fn push_word(&mut self, word: u64) {
        let at = self.word_count * self.word_bits;

        let word_index = (at >> 6) as usize;
        let bit_index = at & 63;

        if bit_index + self.word_bits > 64 {
            self.bm[word_index] |= word << bit_index;
            self.bm[word_index + 1] |= word >> (64 - bit_index);
        } else {
            self.bm[word_index] |= word << bit_index;
        }

        self.word_count += 1
        //
    }

    // returned word is aligned to least significant bits
    pub(crate) fn get_word(&self, i: u64) -> u64 {
        let at = i * self.word_bits;

        let word_index = (at >> 6) as usize;
        let bit_index = at & 63;

        if bit_index + self.word_bits > 64 {
            let v = self.bm[word_index] >> bit_index;
            let left = bit_index + self.word_bits - 64;
            let v2 = self.bm[word_index + 1] & ((1 << left) - 1);
            v | v2 << (64 - bit_index)
        } else {
            (self.bm[word_index] >> bit_index) & ((1 << self.word_bits) - 1)
        }
    }

    pub(crate) fn find(&self, word: u64) -> u64 {
        let mut left: isize = -1;
        let mut right = self.word_count as isize;

        while left < right - 1 {
            let mid = (left + right) / 2;

            let w = self.get_word(mid as u64);

            if word > w {
                left = mid;
            } else {
                right = mid;
            }
        }
        right as u64
    }

    pub(crate) fn find_range(&self, word: u64, from: isize, to: isize) -> u64 {
        let mut left: isize = from - 1;
        let mut right = to;

        while left < right - 1 {
            let mid = (left + right) / 2;

            let w = self.get_word(mid as u64);

            if word > w {
                left = mid;
            } else {
                right = mid;
            }
        }
        right as u64
    }
}

#[cfg(test)]
mod tests {
    use crate::bitmap::Bitmap;

    #[test]
    fn test_new() {
        let bm = Bitmap::new(0, 3);
        assert_eq!(0, bm.bm.len());

        let bm = Bitmap::new(63, 3);
        assert_eq!(1, bm.bm.len());
        assert_eq!(0, bm.bm[0]);

        let bm = Bitmap::new(64, 3);
        assert_eq!(1, bm.bm.len());
        assert_eq!(0, bm.bm[0]);
        assert_eq!(3, bm.word_bits);
        assert_eq!(0, bm.word_count);

        let bm = Bitmap::new(65, 3);
        assert_eq!(2, bm.bm.len());
    }

    #[test]
    fn test_set() {
        let mut bm = Bitmap::new(65, 3);
        bm.set(5);
        bm.set(7);
        bm.set(64);
        bm.set(65);

        assert_eq!(vec![0b10100000, 0b11], bm.bm);
    }

    #[test]
    fn test_push_get_word() {
        {
            // word size =3
            let mut bm = Bitmap::new(64 * 3, 3);
            bm.push_word(0b101);
            bm.push_word(0b111);
            bm.push_word(0b001);

            assert_eq!(vec![0b001111101, 0b0, 0b0], bm.bm);
            assert_eq!(3, bm.word_count);
        }

        {
            // word size = 31
            let mut bm = Bitmap::new(64 * 3, 31);
            bm.push_word(0b101);
            bm.push_word(0b111);
            bm.push_word(0b111);

            assert_eq!(
                vec![
                    (0b1100_0000_0000_0000_0000_0000_0000_0000 << 32)
                        + 0b0011_1000_0000_0000_0000_0000_0000_0000_0101,
                    0b1,
                    0b0
                ],
                bm.bm
            );
            assert_eq!(3, bm.word_count);

            assert_eq!(0b101, bm.get_word(0));
            assert_eq!(0b111, bm.get_word(1));
            assert_eq!(0b111, bm.get_word(2));
            assert_eq!(0b0, bm.get_word(3));
        }
    }

    #[test]
    fn test_find_word() {
        let mut bm = Bitmap::new(64 * 3, 31);
        bm.push_word(0b0101);
        bm.push_word(0b0111);
        bm.push_word(0b1001);

        assert_eq!(0, bm.find(0b0000));
        assert_eq!(0, bm.find(0b0001));
        assert_eq!(0, bm.find(0b0101));

        assert_eq!(1, bm.find(0b0110));
        assert_eq!(1, bm.find(0b0111));

        assert_eq!(2, bm.find(0b1000));
        assert_eq!(2, bm.find(0b1001));

        assert_eq!(3, bm.find(0b1010));
    }

    #[test]
    fn test_find_range() {
        let mut bm = Bitmap::new(64 * 3, 31);
        bm.push_word(0b0101);
        bm.push_word(0b0111);
        bm.push_word(0b1001);

        assert_eq!(0, bm.find_range(0b0001, 0, 1));
        assert_eq!(0, bm.find_range(0b0101, 0, 1));
        assert_eq!(1, bm.find_range(0b0101, 1, 3));

        assert_eq!(1, bm.find_range(0b0110, 0, 1));

        assert_eq!(1, bm.find_range(0b1000, 0, 1));
        assert_eq!(1, bm.find_range(0b1001, 0, 1));

        assert_eq!(2, bm.find_range(0b1010, 0, 2));
    }
}
