use crate::util::bit_count;

#[test]

fn test_bit_count() {
    let n = 3_u64;
    let bb = bit_count(&n);
    println!("{}", bb);
}
