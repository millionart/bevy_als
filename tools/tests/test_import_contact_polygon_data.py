import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_contact_polygon_data", directory / "import_contact_polygon_data.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactPolygonDataTest(unittest.TestCase):
    def test_constants_and_box_face_order(self):
        text = """
#define UE_SMALL_NUMBER (1.e-8f)
FRealDouble SafeNormalize(FRealDouble Epsilon = 1e-4f)
const uint32 MaxContactPointCount = 32;
TArray<TArray<int32>> PlaneVertices
{
    { 0, 4, 6, 2 }, { 0, 1, 5, 4 }, { 0, 2, 3, 1 },
    { 1, 3, 7, 5 }, { 2, 6, 7, 3 }, { 4, 5, 7, 6 }
};
"""
        faces, small, normal, maximum = module.extract([{"text": text}])
        self.assertEqual(faces[2], [0, 2, 3, 1])
        self.assertEqual((small, normal, maximum), (1e-8, 1e-4, 32))
        with self.assertRaises(ValueError):
            module.extract([{"text": text.replace("{ 0, 4, 6, 2 }", "{ 0, 4, 6, 8 }")}])


if __name__ == "__main__":
    unittest.main()
