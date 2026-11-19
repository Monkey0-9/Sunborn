#include "sunborn_simd_accelerator.h"
#include <math.h>

#if defined(__AVX2__)
#include <immintrin.h>
#elif defined(__ARM_NEON) || defined(__ARM_NEON__)
#include <arm_neon.h>
#endif

int sunborn_c_has_hardware_simd(void) {
#if defined(__AVX2__)
    return 1;
#elif defined(__ARM_NEON) || defined(__ARM_NEON__)
    return 1;
#else
    return 0;
#endif
}

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

    size_t i = 0;
    float s_dot = 0.0f;
    float s_norm_a = 0.0f;
    float s_norm_b = 0.0f;

#if defined(__AVX2__)
    __m256 vdot = _mm256_setzero_ps();
    __m256 vnorm_a = _mm256_setzero_ps();
    __m256 vnorm_b = _mm256_setzero_ps();

    for (; i + 7 < n; i += 8) {
        __m256 va = _mm256_loadu_ps(a + i);
        __m256 vb = _mm256_loadu_ps(b + i);

#if defined(__FMA__)
        vdot = _mm256_fmadd_ps(va, vb, vdot);
        vnorm_a = _mm256_fmadd_ps(va, va, vnorm_a);
        vnorm_b = _mm256_fmadd_ps(vb, vb, vnorm_b);
#else
        vdot = _mm256_add_ps(vdot, _mm256_mul_ps(va, vb));
        vnorm_a = _mm256_add_ps(vnorm_a, _mm256_mul_ps(va, va));
        vnorm_b = _mm256_add_ps(vnorm_b, _mm256_mul_ps(vb, vb));
#endif
    }

    float tmp_dot[8], tmp_norm_a[8], tmp_norm_b[8];
    _mm256_storeu_ps(tmp_dot, vdot);
    _mm256_storeu_ps(tmp_norm_a, vnorm_a);
    _mm256_storeu_ps(tmp_norm_b, vnorm_b);

    s_dot = tmp_dot[0] + tmp_dot[1] + tmp_dot[2] + tmp_dot[3] +
            tmp_dot[4] + tmp_dot[5] + tmp_dot[6] + tmp_dot[7];
    s_norm_a = tmp_norm_a[0] + tmp_norm_a[1] + tmp_norm_a[2] + tmp_norm_a[3] +
               tmp_norm_a[4] + tmp_norm_a[5] + tmp_norm_a[6] + tmp_norm_a[7];
    s_norm_b = tmp_norm_b[0] + tmp_norm_b[1] + tmp_norm_b[2] + tmp_norm_b[3] +
               tmp_norm_b[4] + tmp_norm_b[5] + tmp_norm_b[6] + tmp_norm_b[7];

#elif defined(__ARM_NEON) || defined(__ARM_NEON__)
    float32x4_t vdot = vdupq_n_f32(0.0f);
    float32x4_t vnorm_a = vdupq_n_f32(0.0f);
    float32x4_t vnorm_b = vdupq_n_f32(0.0f);

    for (; i + 3 < n; i += 4) {
        float32x4_t va = vld1q_f32(a + i);
        float32x4_t vb = vld1q_f32(b + i);
        vdot = vfmaq_f32(vdot, va, vb);
        vnorm_a = vfmaq_f32(vnorm_a, va, va);
        vnorm_b = vfmaq_f32(vnorm_b, vb, vb);
    }

    s_dot = vgetq_lane_f32(vdot, 0) + vgetq_lane_f32(vdot, 1) +
            vgetq_lane_f32(vdot, 2) + vgetq_lane_f32(vdot, 3);
    s_norm_a = vgetq_lane_f32(vnorm_a, 0) + vgetq_lane_f32(vnorm_a, 1) +
               vgetq_lane_f32(vnorm_a, 2) + vgetq_lane_f32(vnorm_a, 3);
    s_norm_b = vgetq_lane_f32(vnorm_b, 0) + vgetq_lane_f32(vnorm_b, 1) +
               vgetq_lane_f32(vnorm_b, 2) + vgetq_lane_f32(vnorm_b, 3);

#else
    float dot0 = 0.0f, dot1 = 0.0f;
    float norm_a0 = 0.0f, norm_a1 = 0.0f;
    float norm_b0 = 0.0f, norm_b1 = 0.0f;

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
    s_dot = dot0 + dot1;
    s_norm_a = norm_a0 + norm_a1;
    s_norm_b = norm_b0 + norm_b1;
#endif

    // Remainder loop
    for (; i < n; ++i) {
        float x = a[i];
        float y = b[i];
        s_dot += x * y;
        s_norm_a += x * x;
        s_norm_b += y * y;
    }

    *out_dot = s_dot;
    *out_norm_a = s_norm_a;
    *out_norm_b = s_norm_b;
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

    float dot = 0.0f;
    float norm_a = 0.0f;
    float norm_b = 0.0f;

    sunborn_c_vector_dot_and_norms(a, b, n, &dot, &norm_a, &norm_b);
    return dot;
}

float sunborn_c_vector_l2_norm(const float *a, size_t n) {
    if (!a || n == 0) return 0.0f;

    size_t i = 0;
    float sum = 0.0f;

#if defined(__AVX2__)
    __m256 vsum = _mm256_setzero_ps();
    for (; i + 7 < n; i += 8) {
        __m256 va = _mm256_loadu_ps(a + i);
#if defined(__FMA__)
        vsum = _mm256_fmadd_ps(va, va, vsum);
#else
        vsum = _mm256_add_ps(vsum, _mm256_mul_ps(va, va));
#endif
    }
    float tmp[8];
    _mm256_storeu_ps(tmp, vsum);
    sum = tmp[0] + tmp[1] + tmp[2] + tmp[3] + tmp[4] + tmp[5] + tmp[6] + tmp[7];
#elif defined(__ARM_NEON) || defined(__ARM_NEON__)
    float32x4_t vsum = vdupq_n_f32(0.0f);
    for (; i + 3 < n; i += 4) {
        float32x4_t va = vld1q_f32(a + i);
        vsum = vfmaq_f32(vsum, va, va);
    }
    sum = vgetq_lane_f32(vsum, 0) + vgetq_lane_f32(vsum, 1) +
          vgetq_lane_f32(vsum, 2) + vgetq_lane_f32(vsum, 3);
#else
    float sum0 = 0.0f, sum1 = 0.0f;
    for (; i + 7 < n; i += 8) {
        float a0 = a[i+0], a1 = a[i+1], a2 = a[i+2], a3 = a[i+3];
        float a4 = a[i+4], a5 = a[i+5], a6 = a[i+6], a7 = a[i+7];
        sum0 += a0 * a0 + a2 * a2 + a4 * a4 + a6 * a6;
        sum1 += a1 * a1 + a3 * a3 + a5 * a5 + a7 * a7;
    }
    sum = sum0 + sum1;
#endif

    for (; i < n; ++i) {
        float x = a[i];
        sum += x * x;
    }

    return sum > 0.0f ? sqrtf(sum) : 0.0f;
}

float sunborn_c_vector_euclidean_distance(const float *a, const float *b, size_t n) {
    if (!a || !b || n == 0) return 0.0f;

    size_t i = 0;
    float sum = 0.0f;

#if defined(__AVX2__)
    __m256 vsum = _mm256_setzero_ps();
    for (; i + 7 < n; i += 8) {
        __m256 va = _mm256_loadu_ps(a + i);
        __m256 vb = _mm256_loadu_ps(b + i);
        __m256 diff = _mm256_sub_ps(va, vb);
#if defined(__FMA__)
        vsum = _mm256_fmadd_ps(diff, diff, vsum);
#else
        vsum = _mm256_add_ps(vsum, _mm256_mul_ps(diff, diff));
#endif
    }
    float tmp[8];
    _mm256_storeu_ps(tmp, vsum);
    sum = tmp[0] + tmp[1] + tmp[2] + tmp[3] + tmp[4] + tmp[5] + tmp[6] + tmp[7];
#elif defined(__ARM_NEON) || defined(__ARM_NEON__)
    float32x4_t vsum = vdupq_n_f32(0.0f);
    for (; i + 3 < n; i += 4) {
        float32x4_t va = vld1q_f32(a + i);
        float32x4_t vb = vld1q_f32(b + i);
        float32x4_t diff = vsubq_f32(va, vb);
        vsum = vfmaq_f32(vsum, diff, diff);
    }
    sum = vgetq_lane_f32(vsum, 0) + vgetq_lane_f32(vsum, 1) +
          vgetq_lane_f32(vsum, 2) + vgetq_lane_f32(vsum, 3);
#else
    float sum0 = 0.0f, sum1 = 0.0f;
    for (; i + 7 < n; i += 8) {
        float d0 = a[i+0] - b[i+0], d1 = a[i+1] - b[i+1];
        float d2 = a[i+2] - b[i+2], d3 = a[i+3] - b[i+3];
        float d4 = a[i+4] - b[i+4], d5 = a[i+5] - b[i+5];
        float d6 = a[i+6] - b[i+6], d7 = a[i+7] - b[i+7];
        sum0 += d0 * d0 + d2 * d2 + d4 * d4 + d6 * d6;
        sum1 += d1 * d1 + d3 * d3 + d5 * d5 + d7 * d7;
    }
    sum = sum0 + sum1;
#endif

    for (; i < n; ++i) {
        float diff = a[i] - b[i];
        sum += diff * diff;
    }

    return sum > 0.0f ? sqrtf(sum) : 0.0f;
}
