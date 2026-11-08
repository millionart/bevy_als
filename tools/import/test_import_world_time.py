import json
import tempfile
import unittest
from pathlib import Path

from import_world_time import generate


ROOT = Path(__file__).resolve().parents[2]


class WorldTimeImportTests(unittest.TestCase):
    def test_native_world_produces_the_checked_in_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "world-time.json"
            generate(ROOT / "docs/audit/bf144/d06/world.native.json.gz", output)
            expected = ROOT / "assets/ALSExtras/Levels/L_Als_Playground/world_time.runtime.json"
            self.assertEqual(output.read_bytes(), expected.read_bytes())

    def test_invalid_world_bounds_are_rejected_without_output(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "world.json"
            output = Path(directory) / "world-time.json"
            fields = [{"name": "MinUndilatedFrameTime", "numeric_value": 0.4},
                {"name": "MaxUndilatedFrameTime", "numeric_value": 0.0005}]
            source.write_text(json.dumps({"asset_objects": [{"class_path": "/Script/Engine.WorldSettings", "properties": fields}]}))
            with self.assertRaises(ValueError):
                generate(source, output)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
