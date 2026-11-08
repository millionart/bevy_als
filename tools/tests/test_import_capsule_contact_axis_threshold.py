import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_capsule_contact_axis_threshold', directory / 'import_capsule_contact_axis_threshold.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
export_spec = importlib.util.spec_from_file_location('export_capsule_contact_order_probe', directory / 'export_capsule_contact_order_probe.py')
exporter = importlib.util.module_from_spec(export_spec)
export_spec.loader.exec_module(exporter)


class CapsuleContactAxisThresholdTest(unittest.TestCase):
    def test_source_threshold_and_missing_value(self):
        facts = [{'text': 'const FReal CapsuleEndCapAxisNormalThreshold = FReal(0.01);'}]
        self.assertEqual(module.threshold(facts), 0.01)
        with self.assertRaises(ValueError):
            module.threshold([])

    def test_endpoint_axis_conversion(self):
        vector = lambda x, y, z: {'x': x, 'y': y, 'z': z}
        point = {'shape_point_0_cm': vector(0, 0, -1), 'world_normal': vector(1, 0, 0)}
        sample = {'input': {'label': 'sample'}, 'geometry': {'kind': 'capsule',
                  'endpoint_0_cm': vector(0, 0, -1), 'endpoint_1_cm': vector(0, 0, 1)}, 'points': [point, point]}
        profile = {'bodies': [{'bone_name': 'head', 'shapes': [{'target': {'kind': 'capsule',
                   'rotation_xyzw': [0, 0, 0, 1]}}]}]}
        result = exporter.convert({'physics_contact_probe': {'samples': [sample]}}, profile, 'sample', 'head', 'profile.json')
        self.assertAlmostEqual(result['dot'], 1.0)
        self.assertEqual(result['source_points_body_m'][0], [0.0, -0.01, 0.0])


if __name__ == '__main__':
    unittest.main()
