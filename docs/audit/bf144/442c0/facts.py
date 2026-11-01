import hashlib
import json
import os
from pathlib import Path

r = Path(__file__).resolve().parent
config = json.loads(Path("tools/env.local.json").read_text())
plugin = Path(config["ue_project"]).parent / "Plugins/ALS-Refactored"
engine = Path(os.environ["ALS_UE_SOURCE_ROOT"])
units = [
    (plugin / "Source/ALSExtras/Private/AlsCharacterExample.cpp", "source:ALSExtras/Private/AlsCharacterExample.cpp", [(141, 158)]),
    (engine / "Engine/Source/Runtime/Engine/Private/PlayerController.cpp", "Engine/Source/Runtime/Engine/Private/PlayerController.cpp", [(587, 591), (1001, 1032)]),
    (engine / "Engine/Source/Runtime/Engine/Private/PlayerCameraManager.cpp", "Engine/Source/Runtime/Engine/Private/PlayerCameraManager.cpp", [(767, 772)]),
    (Path("src/als_camera/camera_component.rs"), "src/als_camera/camera_component.rs", [(1001, 1025)]),
    (Path("src/als/character/movement/motion.rs"), "src/als/character/movement/motion.rs", [(3070, 3088)]),
]
facts = []
for p, key, ranges in units:
    lines = p.read_text().splitlines()
    facts.append(dict(key=key, sha256=hashlib.sha256(p.read_bytes()).hexdigest(), excerpts=[dict(start=a, end=b, text="\n".join(lines[a - 1:b])) for a, b in ranges]))
(r / "source-facts.json").write_text(json.dumps(facts, indent=2) + "\n")
print("Captured five fresh source/target units", flush=True)
