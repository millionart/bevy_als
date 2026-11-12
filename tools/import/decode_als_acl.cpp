#include <acl/decompression/decompress.h>

#include <algorithm>
#include <array>
#include <bit>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iostream>
#include <iterator>
#include <limits>
#include <stdexcept>
#include <string_view>
#include <vector>

namespace
{
using Pose = std::array<float, 10>;

struct BoneSettings : acl::default_transform_decompression_settings
{
    static constexpr auto version_supported() { return acl::compressed_tracks_version16::latest; }
};

struct KeySettings : BoneSettings
{
    static constexpr auto get_rotation_normalization_policy() { return acl::rotation_normalization_policy_t::never; }
    static constexpr bool is_per_track_rounding_supported() { return true; }
};

struct CurveSettings : acl::decompression_settings
{
    static constexpr auto version_supported() { return acl::compressed_tracks_version16::latest; }
    static constexpr bool is_track_type_supported(acl::track_type8 type) { return type == acl::track_type8::float1f; }
};

struct BoneWriter : acl::track_writer
{
    Pose* bones;

    explicit BoneWriter(Pose* output) : bones(output) {}

    static constexpr auto get_default_rotation_mode() { return acl::default_sub_track_mode::skipped; }
    static constexpr auto get_default_translation_mode() { return acl::default_sub_track_mode::skipped; }
    static constexpr auto get_default_scale_mode() { return acl::default_sub_track_mode::skipped; }

    acl::sample_rounding_policy get_rounding_policy(acl::sample_rounding_policy policy, uint32_t) const
    {
        return policy == acl::sample_rounding_policy::floor ? acl::sample_rounding_policy::none : policy;
    }

    void RTM_SIMD_CALL write_rotation(uint32_t track, rtm::quatf_arg0 value) { rtm::quat_store(value, bones[track].data()); }
    void RTM_SIMD_CALL write_translation(uint32_t track, rtm::vector4f_arg0 value) { rtm::vector_store3(value, bones[track].data() + 4); }
    void RTM_SIMD_CALL write_scale(uint32_t track, rtm::vector4f_arg0 value) { rtm::vector_store3(value, bones[track].data() + 7); }
};

struct CurveWriter : acl::track_writer
{
    float* values;

    explicit CurveWriter(float* output) : values(output) {}

    void RTM_SIMD_CALL write_float1(uint32_t track, rtm::scalarf_arg0 value) { values[track] = rtm::scalar_cast(value); }
};

template<class T>
void write(std::ofstream& output, const T& value)
{
    output.write(reinterpret_cast<const char*>(&value), sizeof(value));
}

template<class Settings>
void initialize(acl::decompression_context<Settings>& context, const acl::compressed_tracks& tracks)
{
    if (!context.initialize(tracks)) { throw std::runtime_error("ACL context initialization failed"); }
}

std::array<float, 8> probe_times(const acl::compressed_tracks& tracks)
{
    const float duration = tracks.get_finite_duration();
    const float interval = 1.0f / tracks.get_sample_rate();
    return {0.0f, 0.37f * interval, 0.63f / tracks.get_sample_rate(), interval, 0.5f * duration,
        (std::max)(0.0f, duration - 0.37f * interval), std::nextafter(duration, 0.0f), duration};
}

void export_bones(std::ofstream& output, const acl::compressed_tracks& tracks, bool single_root, bool step)
{
    const auto& header = acl::acl_impl::get_tracks_header(tracks);
    const auto& transform = acl::acl_impl::get_transform_tracks_header(tracks);
    const bool stripped = tracks.has_stripped_keyframes();
    if (tracks.has_database() || (stripped && !step) ||
        header.get_rotation_format() != acl::rotation_format8::quatf_drop_w_variable ||
        header.get_translation_format() != acl::vector_format8::vector3f_variable ||
        header.get_scale_format() != acl::vector_format8::vector3f_variable)
    {
        throw std::runtime_error("Unsupported ALS transform stream format");
    }
    write(output, uint8_t(header.get_has_scale()));
    write(output, uint8_t(tracks.get_default_scale()));
    write(output, uint8_t(stripped));
    const uint32_t track_count = tracks.get_num_tracks();
    const uint32_t sample_count = tracks.get_num_samples_per_track();
    const uint32_t tracks_per_word = acl::acl_impl::k_num_sub_tracks_per_packed_entry;
    const uint32_t word_count = (track_count + tracks_per_word - 1) / tracks_per_word;
    const auto* types = transform.get_sub_track_types();
    const std::array<uint32_t, 4> offsets{0, 4, 7, 10};

    acl::decompression_context<KeySettings> raw;
    initialize(raw, tracks);
    std::vector<Pose> keys(size_t(track_count) * sample_count);
    for (uint32_t frame = 0; frame < sample_count; ++frame)
    {
        const float rate = tracks.get_sample_rate();
        float time = float(frame) / rate;
        if (stripped && time * rate < float(frame)) { time = std::nextafter(time, std::numeric_limits<float>::infinity()); }
        BoneWriter writer(keys.data() + size_t(frame) * track_count);
        raw.seek(time, stripped ? acl::sample_rounding_policy::floor : acl::sample_rounding_policy::nearest);
        raw.decompress_tracks(writer);
    }

    for (uint32_t track = 0; track < track_count; ++track)
    {
        for (uint32_t channel = 0; channel < 3; ++channel)
        {
            const uint32_t word = channel * word_count + track / tracks_per_word;
            const uint32_t shift = 30 - 2 * (track % tracks_per_word);
            const uint8_t mode = channel == 2 && !header.get_has_scale() ? 0 : (types[word].types >> shift) & 3;
            if (mode > 2) { throw std::runtime_error("Invalid ACL sub-track mode"); }
            write(output, mode);
            const uint32_t count = mode == 0 ? 0 : mode == 1 ? 1 : sample_count;
            for (uint32_t frame = 0; frame < count; ++frame)
            {
                const Pose& values = keys[size_t(frame) * track_count + track];
                for (uint32_t component = offsets[channel]; component < offsets[channel + 1]; ++component)
                {
                    write(output, values[component]);
                }
            }
        }
    }

    acl::decompression_context<BoneSettings> playback;
    initialize(playback, tracks);
    std::vector<Pose> pose(track_count);
    const auto base_times = probe_times(tracks);
    std::vector<float> times(base_times.begin(), base_times.end());
    if (stripped)
    {
        for (uint32_t frame = 0; frame < sample_count; ++frame)
        {
            times.push_back(float(frame) / tracks.get_sample_rate());
            times.push_back((float(frame) + 0.5f) / tracks.get_sample_rate());
        }
    }
    write(output, uint32_t(times.size()));
    for (float time : times)
    {
        write(output, time);
        BoneWriter writer(pose.data());
        playback.seek(time, step ? acl::sample_rounding_policy::floor : acl::sample_rounding_policy::none);
        playback.decompress_tracks(writer);
        if (single_root) { playback.decompress_track(0, writer); }
        for (const Pose& bone : pose) { write(output, bone); }
    }
}

void export_curves(std::ofstream& output, const acl::compressed_tracks& tracks)
{
    const auto* types = acl::acl_impl::get_scalar_tracks_header(tracks).get_track_metadata();
    const uint32_t track_count = tracks.get_num_tracks();
    const uint32_t sample_count = tracks.get_num_samples_per_track();
    acl::decompression_context<CurveSettings> context;
    initialize(context, tracks);
    std::vector<float> keys(size_t(track_count) * sample_count);
    for (uint32_t frame = 0; frame < sample_count; ++frame)
    {
        CurveWriter writer(keys.data() + size_t(frame) * track_count);
        context.seek(float(frame) / tracks.get_sample_rate(), acl::sample_rounding_policy::nearest);
        context.decompress_tracks(writer);
    }
    for (uint32_t track = 0; track < track_count; ++track)
    {
        const uint8_t mode = types[track].bit_rate == 0 ? 1 : 2;
        write(output, mode);
        const uint32_t count = mode == 1 ? 1 : sample_count;
        for (uint32_t frame = 0; frame < count; ++frame)
        {
            write(output, keys[size_t(frame) * track_count + track]);
        }
    }
    std::vector<float> values(track_count);
    const auto times = probe_times(tracks);
    write(output, uint32_t(times.size()));
    for (float time : times)
    {
        write(output, time);
        CurveWriter writer(values.data());
        context.seek(time, acl::sample_rounding_policy::none);
        context.decompress_tracks(writer);
        for (float value : values) { write(output, value); }
    }
}
}

int main(int argc, const char** argv)
{
    static_assert(std::endian::native == std::endian::little && sizeof(float) == 4);
    if (argc != 5)
    {
        std::cerr << "Usage: decode_als_acl input output single_root step\n";
        return 2;
    }
    try
    {
        std::ifstream input(argv[1], std::ios::binary);
        std::vector<char> bytes{std::istreambuf_iterator<char>(input), {}};
        uint32_t size = 0;
        if (bytes.size() < sizeof(acl::compressed_tracks)) { throw std::runtime_error("Truncated ACL stream"); }
        std::memcpy(&size, bytes.data(), sizeof(size));
        if (size != bytes.size()) { throw std::runtime_error("ACL stream size mismatch"); }
        acl::error_result error;
        const auto* tracks = acl::make_compressed_tracks(bytes.data(), &error);
        if (tracks == nullptr || error.any() || tracks->is_valid(true).any() ||
            tracks->get_num_tracks() == 0 || tracks->get_num_samples_per_track() == 0 ||
            !(tracks->get_sample_rate() > 0.0f))
        {
            throw std::runtime_error("Invalid ACL stream");
        }
        std::ofstream output(argv[2], std::ios::binary);
        output.exceptions(std::ios::badbit | std::ios::failbit);
        write(output, tracks->get_num_tracks());
        write(output, tracks->get_num_samples_per_track());
        write(output, tracks->get_sample_rate());
        write(output, tracks->get_finite_duration());
        write(output, uint8_t(tracks->get_looping_policy() == acl::sample_looping_policy::wrap));
        const bool single_root = std::string_view(argv[3]) == "1";
        const bool step = std::string_view(argv[4]) == "1";
        switch (tracks->get_track_type())
        {
            case acl::track_type8::qvvf: export_bones(output, *tracks, single_root, step); break;
            case acl::track_type8::float1f: export_curves(output, *tracks); break;
            default: throw std::runtime_error("Unsupported ACL track type");
        }
        output.close();
    }
    catch (const std::exception& error)
    {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
