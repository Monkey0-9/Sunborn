#ifndef SUNBORN_GRAPH_ACCELERATOR_H
#define SUNBORN_GRAPH_ACCELERATOR_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Performs 4-way unrolled 64-bit word bitwise OR union: dst[i] |= src[i].
 * Returns the total population count (hamming weight) of the resulting union in dst.
 */
size_t sunborn_c_bitset_union_popcount(uint64_t *dst, const uint64_t *src, size_t num_words);

/**
 * Performs 4-way unrolled 64-bit word bitwise AND intersection: dst[i] &= src[i].
 * Returns the total population count of the resulting intersection in dst.
 */
size_t sunborn_c_bitset_intersection_popcount(uint64_t *dst, const uint64_t *src, size_t num_words);

/**
 * Computes total population count across an array of 64-bit words using hardware popcount instructions.
 */
size_t sunborn_c_bitset_popcount(const uint64_t *words, size_t num_words);

/**
 * High-speed 64-bit FNV-1a byte hash for fast zero-allocation key hashing.
 */
uint64_t sunborn_c_fast_hash64(const uint8_t *data, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* SUNBORN_GRAPH_ACCELERATOR_H */
