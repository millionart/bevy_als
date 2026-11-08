import copy
import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_contact_lifecycle_settings", directory / "import_contact_lifecycle_settings.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
export_spec = importlib.util.spec_from_file_location("export_aggregate_contact_probe", directory / "export_aggregate_contact_probe.py")
exporter = importlib.util.module_from_spec(export_spec)
export_spec.loader.exec_module(exporter)


class ContactLifecycleSettingsTest(unittest.TestCase):
    def test_world_default_and_native_overrides(self):
        facts = [{"text": "FRealSingle Chaos_Collision_CullDistanceScaleInverseSize = 0.01f; "
                          "FRealSingle Chaos_Collision_MinCullDistanceScale = 1.0f; "
                          "bool bChaos_Manifold_EnableGjkWarmStart = true;"}]
        settings = {"asset_properties": [{"name": "SolverOptions", "fields": [
            {"name": "CollisionCullDistance", "numeric_value": 3.0}]}]}
        native = {"physics_asset": {"constraint_console_variables": {"p.Chaos.Solver.Collision.CullDistance": "-1",
                  "p.Chaos.Solver.Collision.VelocityBoundsMultiplier": "1",
                  "p.Chaos.Solver.Collision.MaxVelocityBoundsExpansion": "3"}}}
        values, warm = module.parameters(facts, settings, native)
        self.assertEqual(values, [3.0, module.f32(0.01), 1.0, 1.0, 3.0])
        self.assertTrue(warm)
        changed = copy.deepcopy(native)
        changed["physics_asset"]["constraint_console_variables"].update({
            "p.Chaos.Solver.Collision.CullDistance": "5", "p.Chaos.Collision.MinCullDistanceScale": "2",
            "p.Chaos.Collision.Manifold.EnableGjkWarmStart": "false"})
        self.assertEqual(module.parameters(facts, settings, changed), ([5.0, module.f32(0.01), 2.0, 1.0, 3.0], False))
        changed["physics_asset"]["constraint_console_variables"]["p.Chaos.Solver.Collision.CullDistance"] = "nan"
        with self.assertRaises(ValueError):
            module.parameters(facts, settings, changed)

    def test_aggregate_export_keeps_shape_order(self):
        sample = {"input": {"label": "sample"}, "geometry": {}, "other_geometry": {},
                  "aggregate_contacts": [{"mesh_shape_index": 1}, {"mesh_shape_index": 2}]}
        report = {"physics_contact_probe": {"samples": [sample]}}
        self.assertEqual(exporter.convert(report, "sample")["aggregate_contacts"], sample["aggregate_contacts"])
        with self.assertRaises(ValueError):
            exporter.convert(report, "missing")


if __name__ == "__main__":
    unittest.main()
