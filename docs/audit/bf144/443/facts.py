import hashlib
import json
import os
from pathlib import Path

r = Path(__file__).resolve().parent
config = json.loads(Path("tools/env.local.json").read_text())
plugin = Path(config["ue_project"]).parent / "Plugins/ALS-Refactored"
units = [(plugin / "Source/ALS/Private/AlsCharacterMovementComponent.cpp", "source:ALS/Private/AlsCharacterMovementComponent.cpp", [(392,438),(565,581)])]
for name, ranges in (("src/als/character/playable_character.rs",[(25,36)]), ("src/als/character/movement/source_physics_timing.rs",[(14,14),(35,79),(101,154),(164,193)]), ("src/als/character/movement/motion.rs",[(2673,2694),(3746,3768),(3815,3828)]), ("src/als/character/movement/dynamics.rs",[(560,585)])):
    units.append((Path(name),name,ranges))
dep = next(Path("target_game/play/deps").glob("avian3d-*.d")).read_text().split()
for suffix, ranges in (("dynamics/integrator/mod.rs",[(502,523)]),("dynamics/solver/solver_body/plugin.rs",[(263,291)])):
    p = Path(next(v for v in dep if v.endswith("/src/"+suffix)))
    units.append((p,"dependency:avian3d-0.7.0/src/"+suffix,ranges))
facts = []
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text="\n".join(lines[a-1:b]))for a,b in ranges]))
(r/"source-facts.json").write_text(json.dumps(facts,indent=2)+"\n")
print("Captured seven direct movement and integrator source units",flush=True)
