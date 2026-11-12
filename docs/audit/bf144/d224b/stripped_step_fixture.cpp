#include <acl/compression/compress.h>
#include <acl/compression/transform_error_metrics.h>
#include <acl/core/ansi_allocator.h>

#include <fstream>

int main(int argc, const char** argv)
{
    if (argc != 2) { return 2; }
    acl::ansi_allocator allocator;
    acl::track_desc_transformf description;
    description.output_index = 0;
    auto track = acl::track_qvvf::make_reserve(description, allocator, 9, 30.0f);
    for (uint32_t i = 0; i < 9; ++i)
    {
        const float position = float(i * i);
        track[i] = rtm::qvv_set(rtm::quat_identity(), rtm::vector_set(position, -position, 0.5f * position), rtm::vector_set(1.0f));
    }
    acl::track_array_qvvf tracks(allocator, 1);
    tracks[0] = std::move(track);
    auto settings = acl::get_default_compression_settings();
    acl::qvvf_transform_error_metric error_metric;
    settings.error_metric = &error_metric;
    settings.keyframe_stripping.proportion = 0.8f;
    acl::compressed_tracks* output = nullptr;
    acl::output_stats stats;
    if (acl::compress_track_list(allocator, tracks, settings, output, stats).any()) { return 3; }
    std::ofstream file(argv[1], std::ios::binary);
    const bool stripped = output->has_stripped_keyframes();
    file.write(reinterpret_cast<const char*>(output), output->get_size());
    allocator.deallocate(output, output->get_size());
    return stripped && file ? 0 : 4;
}
