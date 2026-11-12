import json
from pathlib import Path
import tempfile
import unittest

from generate_grounded_dynamics_curves import f32, generate


ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / 'docs/audit/bf144/d267'
NATIVE = EVIDENCE / 'curves.native.json'
SETTINGS = {name: EVIDENCE / f'settings_{name}.native.json' for name in ('normal', 'responsive', 'sluggish')}
BASELINE = ROOT / 'docs/audit/bf144/513/runtime-baseline/profile.json'


class GroundedDynamicsCurveTests(unittest.TestCase):
    def test_source_keys_replace_fitted_tangents(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'curves.json'
            report = generate(NATIVE, SETTINGS, BASELINE, output)
            curves = json.loads(output.read_text())
            self.assertEqual(f32(curves['normal']['acceleration']['keys'][2]['leave_tangent']), -36.42372131347656)
            self.assertEqual(f32(curves['responsive']['braking_deceleration']['keys'][2]['leave_tangent']), -27.156404495239258)
            self.assertEqual(curves['normal']['braking_deceleration']['keys'][1]['leave_tangent'], -2.5)
            self.assertEqual(curves['normal']['braking_deceleration']['keys'][1]['interpolation'], 'Linear')
            self.assertEqual(curves['normal']['acceleration']['keys'][2]['interpolation'], 'Cubic')
            self.assertEqual(f32(curves['sluggish']['acceleration']['keys'][2]['leave_tangent'] * 100.0), -843.4028930664062)
            changed = {(row['profile'], row['channel'], row['key'], row['field']) for row in report['changed_fields']}
            self.assertEqual(changed, {
                ('normal', 'acceleration', 2, 'leave_tangent'),
                ('responsive', 'braking_deceleration', 2, 'leave_tangent'),
                ('sluggish', 'acceleration', 2, 'leave_tangent'),
                ('sluggish', 'braking_deceleration', 2, 'arrive_tangent'),
                ('sluggish', 'braking_deceleration', 2, 'leave_tangent'),
                ('sluggish', 'ground_friction', 2, 'arrive_tangent'),
                ('sluggish', 'ground_friction', 2, 'leave_tangent'),
            })

    def test_weighted_curve_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            native = json.loads(NATIVE.read_text())
            native['blend_profile_probe']['samples'][0]['custom_curve_keys'][0]['weight_mode'] = 1
            path = directory / 'native.json'
            path.write_text(json.dumps(native))
            output = directory / 'curves.json'
            with self.assertRaises(ValueError):
                generate(path, SETTINGS, BASELINE, output)
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
