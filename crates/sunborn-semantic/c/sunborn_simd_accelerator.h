#ifndef SUNBORN_SIMD_ACCELERATOR_H
#define SUNBORN_SIMD_ACCELERATOR_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Computes high-performance vectorized cosine similarity between two float vectors of length `n`.
 * Uses 8-way loop unrolling and dual accumulators for superscalar CPU pipeline throughput.
 * Returns 0.0f if length is 0, pointers are NULL, or either vector norm is 0.0f.
 */
float sunborn_c_cosine_similarity(const float *a, const float *b, size_t n);

/**
 * Computes dot product and norms simultaneously in a single memory pass for maximum cache locality.
 */
void sunborn_c_vector_dot_and_norms(
    const float *a,
    const float *b,
    size_t n,
    float *out_dot,
    float *out_norm_a,
    float *out_norm_b
);

/**
 * Computes fast 8-wide unrolled dot product: sum(a[i] * b[i]).
 */
float sunborn_c_vector_dot_product(const float *a, const float *b, size_t n);

/**
 * Computes fast 8-wide unrolled L2 norm (magnitude): sqrt(sum(a[i]^2)).
 */
float sunborn_c_vector_l2_norm(const float *a, size_t n);

/**
 * Computes fast 8-wide unrolled Euclidean distance: sqrt(sum((a[i] - b[i])^2)).
 */
float sunborn_c_vector_euclidean_distance(const float *a, const float *b, size_t n);

#ifdef __cplusplus
}
#endif

#endif /* SUNBORN_SIMD_ACCELERATOR_H */
