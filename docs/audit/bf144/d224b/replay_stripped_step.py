import argparse
import hashlib
import json
import struct
from pathlib import Path

from replay_prepared_acl import HERE, ROOT, sample, selection
from compile_als_prepared_acl import compile_tool, run_tool, unpack_stream


def check(source, sdk, work):
    work.mkdir(parents=True, exist_ok=True)
    report = work / "watchdog.json"
    report.unlink(missing_ok=True)
    fixture = work / "stripped_fixture"
    stream = work / "stripped.compressed.bin"
    decoder = work / "decode_als_acl"
    readback = work / "readback.bin"
    compile_tool(HERE / "stripped_step_fixture.cpp", fixture, sdk, report)
    run_tool([str(fixture.resolve()), str(stream)], report, "generating the stripped Step fixture")

    def decode(implementation):
        compile_tool(implementation, decoder, sdk, report)
        run_tool([str(decoder.resolve()), str(stream), str(readback), "1", "1"], report, "decoding the stripped Step fixture")
        data = readback.read_bytes()
        keys = bytearray()
        clock, tracks, probes = unpack_stream(data, (4, 3, 3), 1, keys)
        assert clock["stripped_step"]
        failures = []
        for time, *pose in struct.iter_unpack("<11f", probes):
            actual = sample(keys, *tracks[0][1], 3, selection(clock, time, step=True), "bone", True)
            expected = tuple(pose[4:7])
            if actual != expected:
                failures.append({"time": time, "actual_translation": actual, "source_translation": expected})
        return data, failures

    source_bytes = source.read_bytes()
    initial, failures = decode(source)
    assert not failures
    ablated_source = work / "decode_without_stripped_step.cpp"
    stripped = b"raw.seek(time, stripped ? acl::sample_rounding_policy::floor : acl::sample_rounding_policy::nearest);"
    assert source_bytes.count(stripped) == 1
    ablated_source.write_bytes(source_bytes.replace(stripped, b"raw.seek(time, acl::sample_rounding_policy::nearest);"))
    _, failures = decode(ablated_source)
    assert failures
    assert source.read_bytes() == source_bytes
    restored, restored_failures = decode(source)
    assert restored == initial and not restored_failures
    return {
        "result": "PASS_STRIPPED_STEP_CODEC_RECOVERY",
        "nearest_key_mismatches": len(failures),
        "first_mismatch": failures[0],
        "restored_sha256": hashlib.sha256(restored).hexdigest(),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--ue-root", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    sdk = args.ue_root / "Engine/Plugins/Animation/ACLPlugin/Source/ThirdParty/acl"
    print(json.dumps(check(ROOT / "tools/import/decode_als_acl.cpp", sdk, args.work), indent=2))
