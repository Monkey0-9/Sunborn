#include "sunborn_simd_accelerator.h"
#include <math.h>

void sunborn_c_vector_dot_and_norms(
    const float *a,
    const float *b,
    size_t n,
    float *out_dot,
    float *out_norm_a,
    float *out_norm_b
) {
    if (!out_dot || !out_norm_a || !out_norm_b) return;

    if (!a || !b || n == 0) {
        *out_dot = 0.0f;
        *out_norm_a = 0.0f;
        *out_norm_b = 0.0f;
        return;
    }

    float dot0 = 0.0f, dot1 = 0.0f;
    float norm_a0 = 0.0f, norm_a1 = 0.0f;
    float norm_b0 = 0.0f, norm_b1 = 0.0f;

    size_t i = 0;
    // 8-wide unrolled SIMD loop for pipelined superscalar execution
    for (; i + 7 < n; i += 8) {
        float a0 = a[i+0], b0 = b[i+0];
        float a1 = a[i+1], b1 = b[i+1];
        float a2 = a[i+2], b2 = b[i+2];
        float a3 = a[i+3], b3 = b[i+3];
        float a4 = a[i+4], b4 = b[i+4];
        float a5 = a[i+5], b5 = b[i+5];
        float a6 = a[i+6], b6 = b[i+6];
        float a7 = a[i+7], b7 = b[i+7];

        dot0 += a0 * b0 + a2 * b2 + a4 * b4 + a6 * b6;
        dot1 += a1 * b1 + a3 * b3 + a5 * b5 + a7 * b7;

        norm_a0 += a0 * a0 + a2 * a2 + a4 * a4 + a6 * a6;
        norm_a1 += a1 * a1 + a3 * a3 + a5 * a5 + a7 * a7;

        norm_b0 += b0 * b0 + b2 * b2 + b4 * b4 + b6 * b6;
        norm_b1 += b1 * b1 + b3 * b3 + b5 * b5 + b7 * b7;
    }

    // Remainder loop
    for (; i < n; ++i) {
        float x = a[i];
        float y = b[i];
        dot0 += x * y;
        norm_a0 += x * x;
        norm_b0 += y * y;
    }

    *out_dot = dot0 + dot1;
    *out_norm_a = norm_a0 + norm_a1;
    *out_norm_b = norm_b0 + norm_b1;
}

float sunborn_c_cosine_similarity(const float *a, const float *b, size_t n) {
    if (!a || !b || n == 0) return 0.0f;

    float dot = 0.0f;
    float norm_a = 0.0f;
    float norm_b = 0.0f;

    sunborn_c_vector_dot_and_norms(a, b, n, &dot, &norm_a, &norm_b);

    if (norm_a <= 0.0f || norm_b <= 0.0f) {
        return 0.0f;
    }

    float denom = sqrtf(norm_a) * sqrtf(norm_b);
    if (denom <= 0.0f) {
        return 0.0f;
    }

    float sim = dot / denom;
    // Strictly clamp within valid cosine domain [-1.0, 1.0]
    if (sim > 1.0f) sim = 1.0f;
    if (sim < -1.0f) sim = -1.0f;
    return sim;
}

float sunborn_c_vector_dot_product(const float *a, const float *b, size_t n) {
    if (!a || !b || n == 0) return 0.0f;

    float dot0 = 0.0f, dot1 = 0.0f;
    size_t i = 0;

    for (; i + 7 < n; i += 8) {
        dot0 += a[i+0] * b[i+0] + a[i+2] * b[i+2] + a[i+4] * b[i+4] + a[i+6] * b[i+6];
        dot1 += a[i+1] * b[i+1] + a[i+3] * b[i+3] + a[i+5] * b[i+5] + a[i+7] * b[i+7];
    }

    for (; i < n; ++i) {
        dot0 += a[i] * b[i];
    }

    return dot0 + dot1;
}

float sunborn_c_vector_l2_norm(const float *a, size_t n) {
    if (!a || n == 0) return 0.0f;

    float sum0 = 0.0f, sum1 = 0.0f;
    size_t i = 0;

    for (; i + 7 < n; i += 8) {
        sum0 += a[i+0] * a[i+0] + a[i+2] * a[i+2] + a[i+4] * a[i+4] + a[i+6] * a[i+6];
        sum1 += a[i+1] * a[i+1] + a[i+3] * a[i+3] + a[i+5] * a[i+5] + a[i+7] * a[i+7];
    }

    for (; i < n; ++i) {
        sum0 += a[i] * a[i];
    }

    float total = sum0 + sum1;
    return total > 0.0f ? sqrtf(total) : 0.0f;
}

float sunborn_c_vector_euclidean_distance(const float *a, const float *b, size_t n) {
    if (!a || !b || n == 0) return 0.0f;

    float sum0 = 0.0f, sum1 = 0.0f;
    size_t i = 0;

    for (; i + 7 < n; i += 8) {
        float d0 = a[i+0] - b[i+0];
        float d1 = a[i+1] - b[i+1];
        float d2 = a[i+2] - b[i+2];
        float d3 = a[i+3] - b[i+3];
        float d4 = a[i+4] - b[i+4];
        float d5 = a[i+5] - b[i+5];
        float d6 = a[i+6] - b[i+6];
        float d7 = a[i+7] - b[i+7];

        sum0 += d0 * d0 + d2 * d2 + d4 * d4 + d6 * d6;
        sum1 += d1 * d1 + d3 * d3 + d5 * d5 + d7 * d7;
    }

    for (; i < n; ++i) {
        float diff = a[i] - b[i];
        sum0 += diff * diff;
    }

    float total = sum0 + sum1;
    return total > 0.0f ? sqrtf(total) : 0.0f;
}
