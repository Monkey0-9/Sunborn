#include "sunborn_graph_accelerator.h"

#if defined(_MSC_VER)
#include <intrin.h>
#define POPCOUNT64(x) __popcnt64(x)
#else
#define POPCOUNT64(x) __builtin_popcountll(x)
#endif

size_t sunborn_c_bitset_union_popcount(uint64_t *dst, const uint64_t *src, size_t num_words) {
    if (!dst || !src || num_words == 0) return 0;

    size_t count = 0;
    size_t i = 0;

    // 4-way unrolled loop for high-throughput bitwise OR and hardware popcount
    for (; i + 3 < num_words; i += 4) {
        uint64_t w0 = (dst[i + 0] |= src[i + 0]);
        uint64_t w1 = (dst[i + 1] |= src[i + 1]);
        uint64_t w2 = (dst[i + 2] |= src[i + 2]);
        uint64_t w3 = (dst[i + 3] |= src[i + 3]);

        count += (size_t)POPCOUNT64(w0) + (size_t)POPCOUNT64(w1) +
                 (size_t)POPCOUNT64(w2) + (size_t)POPCOUNT64(w3);
    }

    // Remainder loop
    for (; i < num_words; ++i) {
        uint64_t w = (dst[i] |= src[i]);
        count += (size_t)POPCOUNT64(w);
    }

    return count;
}

size_t sunborn_c_bitset_intersection_popcount(uint64_t *dst, const uint64_t *src, size_t num_words) {
    if (!dst || !src || num_words == 0) return 0;

    size_t count = 0;
    size_t i = 0;

    for (; i + 3 < num_words; i += 4) {
        uint64_t w0 = (dst[i + 0] &= src[i + 0]);
        uint64_t w1 = (dst[i + 1] &= src[i + 1]);
        uint64_t w2 = (dst[i + 2] &= src[i + 2]);
        uint64_t w3 = (dst[i + 3] &= src[i + 3]);

        count += (size_t)POPCOUNT64(w0) + (size_t)POPCOUNT64(w1) +
                 (size_t)POPCOUNT64(w2) + (size_t)POPCOUNT64(w3);
    }

    for (; i < num_words; ++i) {
        uint64_t w = (dst[i] &= src[i]);
        count += (size_t)POPCOUNT64(w);
    }

    return count;
}

size_t sunborn_c_bitset_popcount(const uint64_t *words, size_t num_words) {
    if (!words || num_words == 0) return 0;

    size_t count = 0;
    size_t i = 0;

    for (; i + 3 < num_words; i += 4) {
        count += (size_t)POPCOUNT64(words[i + 0]) + (size_t)POPCOUNT64(words[i + 1]) +
                 (size_t)POPCOUNT64(words[i + 2]) + (size_t)POPCOUNT64(words[i + 3]);
    }

    for (; i < num_words; ++i) {
        count += (size_t)POPCOUNT64(words[i]);
    }

    return count;
}

uint64_t sunborn_c_fast_hash64(const uint8_t *data, size_t len) {
    if (!data || len == 0) return 0;

    const uint64_t FNV_OFFSET_BASIS = 14695981039346656037ULL;
    const uint64_t FNV_PRIME = 1099511628211ULL;

    uint64_t hash = FNV_OFFSET_BASIS;
    size_t i = 0;

    for (; i + 3 < len; i += 4) {
        hash = (hash ^ (uint64_t)data[i + 0]) * FNV_PRIME;
        hash = (hash ^ (uint64_t)data[i + 1]) * FNV_PRIME;
        hash = (hash ^ (uint64_t)data[i + 2]) * FNV_PRIME;
        hash = (hash ^ (uint64_t)data[i + 3]) * FNV_PRIME;
    }

    for (; i < len; ++i) {
        hash = (hash ^ (uint64_t)data[i]) * FNV_PRIME;
    }

    return hash;
}
