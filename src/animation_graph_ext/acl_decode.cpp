#include <acl/core/sample_rounding_policy.h>
#include <acl/core/compressed_tracks.h>
#include <acl/core/track_writer.h>
#include <acl/decompression/decompress.h>

#include <cmath>
#include <cstddef>
#include <cstdint>

struct AlsAclTransform {
    float rotation[4];
    float translation[3];
    float scale[3];
};

namespace {
const acl::compressed_tracks* tracks(const std::uint8_t* buffer, std::size_t size, bool hash) {
    if (buffer == nullptr || size < sizeof(acl::compressed_tracks)
        || reinterpret_cast<std::uintptr_t>(buffer) % alignof(acl::compressed_tracks) != 0) {
        return nullptr;
    }
    acl::error_result error;
    const auto* data = acl::make_compressed_tracks(buffer, &error);
    if (data == nullptr || data->get_size() != size || data->is_valid(hash).any()) {
        return nullptr;
    }
    return data;
}

template<bool UseBindPose>
struct PoseWriter : acl::track_writer {
    const AlsAclTransform* reference;
    const std::uint8_t* required;
    AlsAclTransform* output;

    PoseWriter(const AlsAclTransform* reference, const std::uint8_t* required, AlsAclTransform* output)
        : reference(reference), required(required), output(output) {}

    static constexpr acl::default_sub_track_mode get_default_rotation_mode() {
        return UseBindPose ? acl::default_sub_track_mode::variable : acl::default_sub_track_mode::constant;
    }
    static constexpr acl::default_sub_track_mode get_default_translation_mode() {
        return UseBindPose ? acl::default_sub_track_mode::variable : acl::default_sub_track_mode::constant;
    }
    static constexpr acl::default_sub_track_mode get_default_scale_mode() {
        return UseBindPose ? acl::default_sub_track_mode::variable : acl::default_sub_track_mode::legacy;
    }

    rtm::quatf RTM_SIMD_CALL get_variable_default_rotation(std::uint32_t track) const {
        return rtm::quat_load(reference[track].rotation);
    }
    rtm::vector4f RTM_SIMD_CALL get_variable_default_translation(std::uint32_t track) const {
        return rtm::vector_load3(reference[track].translation);
    }
    rtm::vector4f RTM_SIMD_CALL get_variable_default_scale(std::uint32_t track) const {
        return rtm::vector_load3(reference[track].scale);
    }

    bool skip_track_rotation(std::uint32_t track) const { return required != nullptr && required[track] == 0; }
    bool skip_track_translation(std::uint32_t track) const { return skip_track_rotation(track); }
    bool skip_track_scale(std::uint32_t track) const { return skip_track_rotation(track); }

    void RTM_SIMD_CALL write_rotation(std::uint32_t index, rtm::quatf_arg0 value) {
        rtm::quat_store(value, output[index].rotation);
    }
    void RTM_SIMD_CALL write_translation(std::uint32_t index, rtm::vector4f_arg0 value) {
        rtm::vector_store3(value, output[index].translation);
    }
    void RTM_SIMD_CALL write_scale(std::uint32_t index, rtm::vector4f_arg0 value) {
        rtm::vector_store3(value, output[index].scale);
    }
};

struct CurveWriter : acl::track_writer {
    float* output;
    explicit CurveWriter(float* output) : output(output) {}

    void RTM_SIMD_CALL write_float1(std::uint32_t index, rtm::scalarf_arg0 value) {
        output[index] = rtm::scalar_cast(value);
    }
};
}

extern "C" std::uint32_t als_acl_validate(
    const std::uint8_t* bytes, std::size_t size, bool transform, bool additive
) {
    const auto* clip = tracks(bytes, size, true);
    const auto kind = transform ? acl::track_type8::qvvf : acl::track_type8::float1f;
    if (clip == nullptr || clip->get_track_type() != kind
        || (transform && (clip->get_default_scale() == 0) != additive)) {
        return 0;
    }
    return clip->get_num_tracks();
}

extern "C" bool als_acl_decode_pose(
    const std::uint8_t* bytes, std::size_t size, float time, bool step,
    const AlsAclTransform* reference, const std::uint8_t* required,
    AlsAclTransform* output, std::uint32_t count
) {
    const auto* clip = tracks(bytes, size, false);
    if (clip == nullptr || clip->get_track_type() != acl::track_type8::qvvf
        || clip->get_num_tracks() != count || !std::isfinite(time)
        || reference == nullptr || output == nullptr) {
        return false;
    }
    acl::decompression_context<acl::default_transform_decompression_settings> context;
    if (!context.initialize(*clip)) {
        return false;
    }
    context.seek(time, step ? acl::sample_rounding_policy::floor : acl::sample_rounding_policy::none);
    if (clip->get_default_scale() != 0) {
        PoseWriter<true> writer(reference, required, output);
        context.decompress_tracks(writer);
    } else {
        PoseWriter<false> writer(reference, required, output);
        context.decompress_tracks(writer);
    }
    return true;
}

extern "C" bool als_acl_decode_curves(
    const std::uint8_t* bytes, std::size_t size, float time,
    const std::uint8_t* required, float* output, std::uint32_t count
) {
    const auto* clip = tracks(bytes, size, false);
    if (clip == nullptr || clip->get_track_type() != acl::track_type8::float1f
        || clip->get_num_tracks() != count || !std::isfinite(time) || output == nullptr) {
        return false;
    }
    acl::decompression_context<acl::default_scalar_decompression_settings> context;
    if (!context.initialize(*clip)) {
        return false;
    }
    context.seek(time, acl::sample_rounding_policy::none);
    CurveWriter writer(output);
    if (required == nullptr) {
        context.decompress_tracks(writer);
    } else {
        for (std::uint32_t track = 0; track < count; ++track) {
            if (required[track] != 0) {
                context.decompress_track(track, writer);
            }
        }
    }
    return true;
}
