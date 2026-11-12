#include <arm_neon.h>

namespace
{
float64x2_t dot(float64x2_t xy0, float64x2_t zw0, float64x2_t xy1, float64x2_t zw1)
{
    const auto pairs = vfmaq_f64(vmulq_f64(xy0, xy1), zw0, zw1);
    return vaddq_f64(pairs, vextq_f64(pairs, pairs, 1));
}

void normalize(float64x2_t xy, float64x2_t zw, double* output)
{
    const auto squared = dot(xy, zw, xy, zw);
    auto reciprocal = vrsqrteq_f64(squared);
    reciprocal = vmulq_f64(vrsqrtsq_f64(squared, vmulq_f64(reciprocal, reciprocal)), reciprocal);
    reciprocal = vmulq_f64(vrsqrtsq_f64(squared, vmulq_f64(reciprocal, reciprocal)), reciprocal);
    vst1q_f64(output, vmulq_f64(xy, reciprocal));
    vst1q_f64(output + 2, vmulq_f64(zw, reciprocal));
}
}

extern "C" void source_normalize(const double* value, double* output)
{
    normalize(vld1q_f64(value), vld1q_f64(value + 2), output);
}

extern "C" void source_nlerp(const double* a, const double* b, double alpha, double* output)
{
    auto xy = vld1q_f64(a);
    auto zw = vld1q_f64(a + 2);
    const auto bxy = vld1q_f64(b);
    const auto bzw = vld1q_f64(b + 2);
    if (vgetq_lane_f64(dot(xy, zw, bxy, bzw), 0) < 0.0)
    {
        xy = vnegq_f64(xy);
        zw = vnegq_f64(zw);
    }
    normalize(vfmaq_n_f64(vmulq_n_f64(bxy, alpha), xy, 1.0 - alpha),
              vfmaq_n_f64(vmulq_n_f64(bzw, alpha), zw, 1.0 - alpha), output);
}
