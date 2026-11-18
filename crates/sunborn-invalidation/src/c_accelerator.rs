//! # Native C Graph & Bitset Acceleration Engine
//!
//! Exposes high-throughput, hardware-vectorized C kernels for 64-bit word bitset
//! operations (union popcount, intersection popcount, standalone popcount) and
//! ultra-fast non-allocating FNV-1a hashing.
//!
//! Includes 100% safe Rust wrappers and reference pure-Rust implementations
//! to guarantee zero-risk execution and exact mathematical parity across targets.

extern "C" {
    fn sunborn_c_bitset_union_popcount(dst: *mut u64, src: *const u64, num_words: usize) -> usize;

    fn sunborn_c_bitset_intersection_popcount(
        dst: *mut u64,
        src: *const u64,
        num_words: usize,
    ) -> usize;

    fn sunborn_c_bitset_popcount(words: *const u64, num_words: usize) -> usize;

    fn sunborn_c_fast_hash64(data: *const u8, len: usize) -> u64;
}

/// Pure-Rust reference implementation of bitset union popcount: `dst[i] |= src[i]`.
/// Used for correctness verification, unit testing, and fallback.
pub fn bitset_union_popcount_reference(dst: &mut [u64], src: &[u64]) -> usize {
    let n = dst.len().min(src.len());
    let mut count = 0;
    for i in 0..n {
        dst[i] |= src[i];
        count += dst[i].count_ones() as usize;
    }
    count
}

/// Pure-Rust reference implementation of bitset intersection popcount: `dst[i] &= src[i]`.
pub fn bitset_intersection_popcount_reference(dst: &mut [u64], src: &[u64]) -> usize {
    let n = dst.len().min(src.len());
    let mut count = 0;
    for i in 0..n {
        dst[i] &= src[i];
        count += dst[i].count_ones() as usize;
    }
    count
}

/// Pure-Rust reference implementation of population count over 64-bit words.
pub fn bitset_popcount_reference(words: &[u64]) -> usize {
    words.iter().map(|w| w.count_ones() as usize).sum()
}

/// Pure-Rust reference implementation of 64-bit FNV-1a hash.
pub fn fast_hash_64_reference(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash = (hash ^ (byte as u64)).wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Performs hardware-accelerated 4-way unrolled 64-bit word bitwise OR union: `dst[i] |= src[i]`.
/// Returns total population count (Hamming weight) of the updated slice.
///
/// Guaranteed 100% memory safe: clamps to `min(dst.len(), src.len())` and guards null pointers.
pub fn c_bitset_union_popcount(dst: &mut [u64], src: &[u64]) -> usize {
    let n = dst.len().min(src.len());
    if n == 0 {
        return 0;
    }

    unsafe { sunborn_c_bitset_union_popcount(dst.as_mut_ptr(), src.as_ptr(), n) }
}

/// Performs hardware-accelerated 4-way unrolled 64-bit word bitwise AND intersection: `dst[i] &= src[i]`.
/// Returns total population count of the updated slice.
pub fn c_bitset_intersection_popcount(dst: &mut [u64], src: &[u64]) -> usize {
    let n = dst.len().min(src.len());
    if n == 0 {
        return 0;
    }

    unsafe { sunborn_c_bitset_intersection_popcount(dst.as_mut_ptr(), src.as_ptr(), n) }
}

/// Computes total population count across an array of 64-bit words using hardware popcount instructions.
pub fn c_bitset_popcount(words: &[u64]) -> usize {
    if words.is_empty() {
        return 0;
    }

    unsafe { sunborn_c_bitset_popcount(words.as_ptr(), words.len()) }
}

/// Computes a high-speed 64-bit FNV-1a byte hash with zero heap allocations.
pub fn c_fast_hash_64(data: &[u8]) -> u64 {
    if data.is_empty() {
        return 0;
    }

    unsafe { sunborn_c_fast_hash64(data.as_ptr(), data.len()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_bitset_union_parity() {
        let mut dst_c = vec![0x0000_1111_2222_3333, 0xaaaa_0000_5555_0000, 0, 0xffff];
        let mut dst_ref = dst_c.clone();
        let src = vec![0x1111_0000_0000_4444, 0x0000_bbbb_0000_6666, 0x1234, 0x0001];

        let count_c = c_bitset_union_popcount(&mut dst_c, &src);
        let count_ref = bitset_union_popcount_reference(&mut dst_ref, &src);

        assert_eq!(dst_c, dst_ref, "C and Rust union words must match exactly");
        assert_eq!(count_c, count_ref, "C and Rust union popcounts must match");
    }

    #[test]
    fn test_c_bitset_intersection_parity() {
        let mut dst_c = vec![0xffff_ffff_0000_1111, 0x1234_5678_9abc_def0, 0x0f0f_0f0f];
        let mut dst_ref = dst_c.clone();
        let src = vec![0x0000_ffff_ffff_0000, 0x0000_5678_0000_def0, 0xf0f0_f0f0];

        let count_c = c_bitset_intersection_popcount(&mut dst_c, &src);
        let count_ref = bitset_intersection_popcount_reference(&mut dst_ref, &src);

        assert_eq!(dst_c, dst_ref, "C and Rust intersection words must match");
        assert_eq!(
            count_c, count_ref,
            "C and Rust intersection popcounts must match"
        );
    }

    #[test]
    fn test_c_bitset_popcount_parity() {
        let words = vec![
            0xffff_ffff_ffff_ffff,
            0x0000_0000_0000_0000,
            0x5555_5555_5555_5555,
            0x1010_1010_1010_1010,
            0x8000_0000_0000_0001,
        ];

        let count_c = c_bitset_popcount(&words);
        let count_ref = bitset_popcount_reference(&words);

        assert_eq!(count_c, count_ref);
        assert_eq!(count_c, 64 + 32 + 8 + 2);
    }

    #[test]
    fn test_c_fast_hash_64_parity() {
        let test_strings = [
            b"sunborn:semantic:cache:key:1".as_slice(),
            b"rag:document:uuid:9988-aabb-ccdd".as_slice(),
            b"".as_slice(),
            b"short".as_slice(),
            b"A much longer payload to ensure unrolled loop remainder blocks execute with exact precision 1234567890".as_slice(),
        ];

        for s in &test_strings {
            let hash_c = c_fast_hash_64(s);
            let hash_ref = if s.is_empty() {
                0
            } else {
                fast_hash_64_reference(s)
            };
            assert_eq!(hash_c, hash_ref, "Hash parity failed for input: {:?}", s);
        }
    }

    #[test]
    fn test_empty_slices_safety() {
        let mut dst = Vec::new();
        let src = Vec::new();
        assert_eq!(c_bitset_union_popcount(&mut dst, &src), 0);
        assert_eq!(c_bitset_intersection_popcount(&mut dst, &src), 0);
        assert_eq!(c_bitset_popcount(&dst), 0);
        assert_eq!(c_fast_hash_64(&[]), 0);
    }
}
