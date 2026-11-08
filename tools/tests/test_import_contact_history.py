import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_contact_history_settings', directory / 'import_contact_history_settings.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
export_spec = importlib.util.spec_from_file_location('export_contact_history_probe', directory / 'export_contact_history_probe.py')
exporter = importlib.util.module_from_spec(export_spec)
export_spec.loader.exec_module(exporter)


class ContactHistorySettingsTest(unittest.TestCase):
    def test_source_matching_distances_and_mode(self):
        facts = [{'text': '#define UE_KINDA_SMALL_NUMBER (1.e-4f)'}]
        native = {'p.Chaos.Collision.Manifold.EnableFrictionRestore': 'true',
                  'p.Chaos.Collision.SimpleAssignContact': 'true',
                  'p.Chaos.Collision.Manifold.FrictionExactPositionTolerance': '0.2',
                  'p.Chaos.Collision.Manifold.FrictionNearPositionTolerance': '1'}
        exact, near, ratio = module.parameters(facts, native)
        self.assertAlmostEqual(exact, 0.002)
        self.assertAlmostEqual(near, 0.01)
        self.assertAlmostEqual(ratio, 0.0001)
        native['p.Chaos.Collision.SimpleAssignContact'] = 'false'
        with self.assertRaises(ValueError):
            module.parameters(facts, native)

    def test_probe_coordinates_and_matching_flags(self):
        first = {'x': 100, 'y': 200, 'z': 300}
        second = {'x': 0, 'y': 0, 'z': 0}
        contact = {'shape_point_0_cm': first, 'shape_point_1_cm': second}
        history = {'shape_anchor_0_cm': first, 'shape_anchor_1_cm': second, 'has_static_friction_anchor': True}
        sample = {'input': {'label': 'case', 'history': {'friction_ratios': [1],
                  'anchor_offset_0_cm': [10, 20, 30], 'anchor_offset_1_cm': [0, 0, 0]}},
                  'geometry': {'kind': 'capsule'}, 'points': [contact], 'saved_points': [contact],
                  'next_points': [contact], 'next_history': [history]}
        result = exporter.convert({'physics_contact_probe': {'samples': [sample]}})['samples'][0]
        self.assertEqual(result['current'][0][0], [1.0, 3.0, 2.0])
        self.assertEqual(result['quadratic'], [True, False])
        self.assertEqual(result['next_matches'], [True])


if __name__ == '__main__':
    unittest.main()
