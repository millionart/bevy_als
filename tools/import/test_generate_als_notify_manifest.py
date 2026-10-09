import tempfile
import unittest
from pathlib import Path

from generate_als_notify_manifest import ValidationError, manifest, source_object_from_reference


def event(event_id="00000000-0000-0000-0000-000000000001", start=0.25, end=0.25):
    return {
        "id": event_id,
        "kind": "point",
        "name": "TestNotify",
        "start_time": start,
        "end_time": end,
        "track_name": "Notify",
        "track_index": 0,
        "weight_threshold": 0.5,
        "filter_type": "NoFiltering",
        "filter_lod": 0,
        "trigger_on_follower": False,
        "trigger_on_dedicated_server": False,
        "trigger_chance": 1.0,
        "montage_tick_type": "Queued",
        "notify": {"class": "TestNotify", "properties": {}},
        "notify_state": None,
        "linked_sequence": "Test",
        "linked_montage": None,
    }


def sequence(source, target, events=None):
    return {
        "source_object": source,
        "asset_class": "AnimSequence",
        "target_gltf": target,
        "sequence_length": 1.0,
        "notifies": events if events is not None else [],
        "markers": [],
        "montage_segments": [],
    }


class NotifyManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.assets = Path(self.temp.name)
        clip = self.assets / "animations/Test/Test.anim.ron"
        clip.parent.mkdir(parents=True)
        clip.write_text("()", encoding="utf-8")
        self.source = "/ALS/Animations/Test/Test.Test"

    def tearDown(self):
        self.temp.cleanup()

    def test_point_event_preserves_exact_time(self):
        result = manifest({"animations": [sequence(self.source, "assets/animations/Test/Test.gltf", [event()])]}, self.assets)
        output = result["clips"][0]["events"][0]
        self.assertEqual(output["start_time"], 0.25)
        self.assertEqual(output["end_time"], 0.25)
        self.assertTrue(output["event_id"].endswith(":00000000-0000-0000-0000-000000000001"))

    def test_notify_times_include_both_native_trigger_offsets(self):
        for kind, start, end, start_offset, end_offset in (
            ("state", 0.0, 0.933333, 0.0001, 0.0),
            ("state", 0.0, 0.966667, -0.0001, 0.0),
            ("state", 1.0, 1.5, 0.0, -0.0001),
            ("point", 0.25, 0.25, -0.0001, 0.0),
        ):
            value = event(start=start, end=end)
            value.update(kind=kind, trigger_time_offset=start_offset, end_trigger_time_offset=end_offset)
            row = sequence(self.source, "assets/animations/Test/Test.gltf", [value])
            row["sequence_length"] = 2.0
            output = manifest({"animations": [row]}, self.assets)["clips"][0]["events"][0]
            self.assertAlmostEqual(output["start_time"], start + start_offset)
            self.assertAlmostEqual(output["end_time"], end + start_offset + end_offset)

    def test_duplicate_ids_are_rejected(self):
        duplicate = sequence(self.source, "assets/animations/Test/Test.gltf", [event(), event()])
        with self.assertRaisesRegex(ValidationError, "duplicate_event_id"):
            manifest({"animations": [duplicate]}, self.assets)

    def test_lod_filter_thresholds_and_tick_types_are_preserved(self):
        for filter_type in ("NoFiltering", "LOD", "ENotifyFilterType::LOD"):
            for lod in (0, 1, 2):
                for tick_type in ("Queued", "BranchingPoint", "EMontageNotifyTickType::BranchingPoint"):
                    value = event()
                    value.update(filter_type=filter_type, filter_lod=lod, montage_tick_type=tick_type)
                    result = manifest({"animations": [sequence(self.source, "assets/animations/Test/Test.gltf", [value])]}, self.assets)
                    actual = result["clips"][0]["events"][0]["filter"]
                    self.assertEqual((actual["type"], actual["lod"], actual["montage_tick_type"]), (filter_type, lod, tick_type))

    def test_unsupported_notify_filter_configuration_is_rejected(self):
        for field, value, error in (("filter_type", "Unknown", "unsupported_notify_filter"), ("montage_tick_type", "Unknown", "unsupported_notify_tick_type"), ("filter_lod", 0.5, "invalid_notify_filter_lod"), ("filter_lod", True, "invalid_notify_filter_lod"), ("filter_lod", 2**31, "invalid_notify_filter_lod")):
            notify = event()
            notify[field] = value
            with self.assertRaisesRegex(ValidationError, error):
                manifest({"animations": [sequence(self.source, "assets/animations/Test/Test.gltf", [notify])]}, self.assets)

    def test_invalid_event_range_is_rejected(self):
        invalid = sequence(self.source, "assets/animations/Test/Test.gltf", [event(start=0.9, end=1.1)])
        with self.assertRaisesRegex(ValidationError, "event_time_out_of_range"):
            manifest({"animations": [invalid]}, self.assets)

    def test_invalid_interval_is_rejected(self):
        invalid = sequence(self.source, "assets/animations/Test/Test.gltf", [event(start=0.8, end=0.2)])
        with self.assertRaisesRegex(ValidationError, "event_time_out_of_range"):
            manifest({"animations": [invalid]}, self.assets)

    def test_missing_graph_clip_is_rejected(self):
        (self.assets / "animations/Test/Test.anim.ron").unlink()
        with self.assertRaisesRegex(ValidationError, "missing_graph_clip"):
            manifest({"animations": [sequence(self.source, "assets/animations/Test/Test.gltf")]}, self.assets)

    def test_montage_maps_to_exported_segment_clip(self):
        base = sequence(self.source, "assets/animations/Test/Test.gltf")
        montage = {
            "source_object": "/ALS/Animations/Test/AM_Test.AM_Test",
            "asset_class": "AnimMontage",
            "target_gltf": None,
            "sequence_length": 1.0,
            "notifies": [event()],
            "markers": [],
            "montage_segments": [
                {
                    "animation": "None//ALS/Animations/Test/Test/Test",
                    "slot": "PostLocomotion",
                    "start_position": 0.0,
                    "animation_start_time": 0.0,
                    "animation_end_time": 1.0,
                    "play_rate": 1.0,
                    "loop_count": 1,
                }
            ],
        }
        result = manifest({"animations": [base, montage]}, self.assets)
        self.assertEqual(result["clips"][1]["runtime_segments"][0]["graph_clip"], "animations/Test/Test.anim.ron")

    def test_reference_parser_uses_virtual_source_path(self):
        self.assertEqual(
            source_object_from_reference("None//ALS/Animations/Test/Test/Test"),
            self.source,
        )


if __name__ == "__main__":
    unittest.main()
