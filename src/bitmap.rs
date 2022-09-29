use crate::next_multiple_of;

pub(crate) struct Bitmap {
    word_bits: u64,
    word_count: u64,
    bm: Vec<u64>,
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

    pub(crate) fn set(&mut self, i: u64) {
        let word_index = i >> 6;
        let bit_index = i & 63;
        self.bm[word_index] |= 1 << bit_index;
    }

    // word is aligned to least significant bits
    pub(crate) fn push_word(&mut self, word: u64) {
        let at = self.word_count * self.word_bits;

        let word_index = at >> 6;
        let bit_index = at & 63;

        if bit_index + word_bits > 64 {
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

        let word_index = at >> 6;
        let bit_index = at & 63;

        if bit_index + word_bits > 64 {
            let v = self.bm[word_index] >> bit_index;
            let left = (word_bits + 1 - bit_index);
            let v2 = self.bm[word_index + 1] & ((1 << left) - 1);
            v | v2 << (64 - bit_index)
        } else {
            (self.bm[word_index] >> bit_index) & ((1 << word_bits) - 1)
        }
    }

    pub(crate) fn find(&self, word: u64) -> (u64, u64) {
        //
        let mut left = 0;
        let mut right = self.word_count;
        let mut w = 0;

        while left < right - 1 {
            let mid = (left + right) / 2;

            w = self.get_word(mid);

            if word > w {
                left = mid;
            } else {
                right = mid;
            }
        }
        (right, w)
    }
}
