import json
from pathlib import Path
import tempfile
import unittest

from audit_animation_transform_conversion import animation_channel_values, load_gltf
from generate_animation_unit_projection import generate


ROOT = Path(__file__).resolve().parents[2]
RAW = ROOT / 'docs/audit/bf144/d275/raw.native.json'
COMPRESSED = ROOT / 'docs/audit/bf144/d275/compressed.native.json'
GLTF = ROOT / 'assets/ALS/Animations/Base/A_Als_Stand_Pose.gltf'


class AnimationUnitProjectionTests(unittest.TestCase):
    def test_source_equal_scalars_use_physical_units_and_other_data_stays_unchanged(self):
        original_gltf = GLTF.read_bytes()
        document, original = load_gltf(GLTF)
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / GLTF.name
            report = generate(RAW, COMPRESSED, GLTF, output)
            derived, data = load_gltf(output)
            self.assertEqual((report['frames'], report['bones']), (2, 79))
            self.assertEqual((report['equal_source_scalars'], report['different_source_scalars'], report['changed_scalars']), (148, 326, 22))
            self.assertEqual(GLTF.read_bytes(), original_gltf)
            self.assertEqual(GLTF.with_suffix('.bin').read_bytes(), original)
            self.assertEqual(len(data), len(original))
            changed_bytes = {index for index, (before, after) in enumerate(zip(original, data)) if before != after}
            allowed_bytes = {index for row in report['changes'] for index in range(row['byte_offset'], row['byte_offset'] + 4)}
            self.assertTrue(changed_bytes)
            self.assertLessEqual(changed_bytes, allowed_bytes)
            self.assertEqual(document['animations'], derived['animations'])
            original_channel = animation_channel_values(document, original, 'pelvis')['translation']
            projected_channel = animation_channel_values(derived, data, 'pelvis')['translation']
            self.assertEqual(original_channel[0], projected_channel[0])
            self.assertEqual(original_channel[1][0][1], 0.9675059914588928)
            self.assertEqual(projected_channel[1][0][1], 0.9675060510635376)
            self.assertEqual(projected_channel[1][0][0], original_channel[1][0][0])
            self.assertEqual(projected_channel[1][0][2], original_channel[1][0][2])

    def test_mismatched_source_identity_is_rejected_before_output(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            wrong = json.loads(COMPRESSED.read_text())
            wrong['asset_path'] = '/ALS/Other.Other'
            path = directory / 'wrong.json'
            path.write_text(json.dumps(wrong))
            output = directory / GLTF.name
            with self.assertRaises(ValueError):
                generate(RAW, path, GLTF, output)
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
