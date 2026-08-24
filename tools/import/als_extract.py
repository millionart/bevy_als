"""Extract L_Als_Playground scene data directly from .uasset/.umap files.

Outputs JSON bundles under tmp/export/als_extract/:
  scene.json        actors (transform/mesh/text/light/materials)
  materials.json    material instances with scalar/vector/texture params
  textures.txt      referenced texture package paths
"""

import importlib.util
import json
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)

from uasset_base import UAsset  # noqa: E402
import uasset_reader as UR  # noqa: E402

REPO = os.path.abspath(os.path.join(_HERE, "..", ".."))
ALS_CONTENT = "D:/Users/milli/Git/ALS-Refactored/Plugins/ALS-Refactored/Content/ALS/ALSExtras"
OUT_DIR = os.path.join(REPO, "tmp", "export", "als_extract")

UMAP = os.path.join(
    ALS_CONTENT.replace("/Content", ""),
    "Content",
    "..",
    "..",
)
UMAP_PATH = os.path.normpath(
    os.path.join(ALS_CONTENT, "..", "..", "Content")
)

LEVEL_UMAP = None
for cand in [
    os.path.join(ALS_CONTENT, "..", "..", "Content", "__None"),
]:
    pass


PLUGIN_CONTENT = os.path.dirname(ALS_CONTENT)
EA_ROOT = os.path.join(PLUGIN_CONTENT, "__ExternalActors__", "ALSExtras", "Levels", "L_Als_Playground")

def find_actor_files():
    files = []
    for root, dirs, fnames in os.walk(EA_ROOT):
        for f in sorted(fnames):
            if f.endswith(".uasset"):
                files.append(os.path.join(root, f))
    return files


def actor_class_name(asset, index):
    cn = asset.class_of_export(index)
    return cn


def unwrap_value(v):
    if isinstance(v, dict) and "__skipped__" in v:
        return None
    if isinstance(v, dict) and v.get("enum"):
        return v.get("value")
    if isinstance(v, dict) and set(v.keys()) == {"x", "y", "z"}:
        return v
    return v


def get_vec(props, key):
    v = props.get(key)
    if isinstance(v, dict) and "x" in v:
        return v
    return {"x": 0.0, "y": 0.0, "z": 0.0}


def main():
    actor_files = find_actor_files()
    print("external actors: %d" % len(actor_files))

    all_actors = []
    all_components = {}
    textures_seen = {}

    for path in actor_files:
        try:
            asset = UAsset(path)
        except Exception as exc:
            print("  skip (parse): %s -> %s" % (os.path.basename(path), str(exc)[:80]))
            continue

        for idx in range(len(asset.exports)):
            cls = asset.class_of_export(idx)
            exp = asset.exports[idx]
            interesting_component = cls in (
                "StaticMeshComponent",
                "TextRenderComponent",
                "LandscapeComponent",
                "LandscapeHeightfieldCollisionComponent",
                "DirectionalLightComponent",
                "RotatingMovementComponent",
                "InterpToMovementComponent",
                "PhysicalAnimationComponent",
            )
            if not interesting_component:
                continue
            pr = UR.PropReader(asset)
            props, ok, err = pr.read_export(idx)
            entry = {
                "file": os.path.basename(path),
                "class": cls,
                "name": exp.object_name,
                "ok": ok,
                "err": err,
            }
            for k in ("RelativeLocation", "RelativeRotation", "RelativeScale3D",
                      "StaticMesh", "Text", "TextColor", "WorldSize",
                      "Intensity", "LightColor", "bUseTemperature", "Temperature",
                      "bAtmosphereSunLight", "HeightmapTexture",
                      "ComponentSizeQuads", "OverrideMaterials"):
                if k in props:
                    entry[k] = props[k]
            all_components[str(len(all_components)) + "_" + exp.object_name] = entry

        # find the top-level actor export
        for idx in range(len(asset.exports)):
            cls = asset.class_of_export(idx)
            if cls is None or cls in ("SceneComponent", "StaticMeshComponent",
                                       "TextRenderComponent", "BillboardComponent",
                                       "ArrowComponent", "BoxComponent"):
                continue
            if cls.startswith("Default__") or cls.startswith("Sprite"):
                continue
            pr = UR.PropReader(asset)
            props, ok, err = pr.read_export(idx)
            entry = {
                "file": os.path.basename(path),
                "class": cls,
                "name": exp.object_name,
                "ok": ok,
            }
            for k in ("RootComponent", "bStatic"):
                if k in props:
                    entry[k] = props[k]
            all_actors.append(entry)
            break

    out = {
        "actors": all_actors,
        "components": all_components,
        "textures_seen": textures_seen,
        "total_files": len(actor_files),
    }
    os.makedirs(OUT_DIR, exist_ok=True)
    out_path = os.path.join(OUT_DIR, "scene_umap.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=1)
    print("wrote", out_path)
    print("actors:", len(all_actors), "components:", len(all_components))

