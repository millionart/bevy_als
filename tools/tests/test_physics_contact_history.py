import json
import math
import sys
from pathlib import Path

data=json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))
settings=data["physics_asset"]["constraint_console_variables"]
assert settings["p.Chaos.Collision.SimpleAssignContact"]=="true"
samples=data["physics_contact_probe"]["samples"]
def rotated(q,v):
    x,y,z,w=q
    cross=lambda a,b:[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
    uv=cross([x,y,z],v);uuv=cross([x,y,z],uv)
    return [v[i]+2*(w*uv[i]+uuv[i]) for i in range(3)]
counts=set()
matched=0
unmatched=0
for sample in samples:
    points=sample["points"];history=sample["input"]["history"];counts.add(len(points))
    expected=[]
    for point,ratio in zip(points,history["friction_ratios"],strict=True):
        if ratio<1e-4 and len(points)>=8:continue
        value={}
        factor=0 if ratio<1e-4 else 1 if ratio>=1-1e-4 else ratio
        for side in [0,1]:
            key=f"shape_point_{side}_cm"
            value[key]={axis:point[key][axis]+factor*history[f"anchor_offset_{side}_cm"][i] for i,axis in enumerate("xyz")}
        expected.append(value)
    saved=sample["saved_points"]
    assert len(saved)==len(expected)
    for actual,wanted in zip(saved,expected,strict=True):
        assert actual["initial_phi_cm"]==0
        for side in [0,1]:
            key=f"shape_point_{side}_cm"
            assert max(abs(actual[key][a]-wanted[key][a]) for a in "xyz")<2e-5
    for index,(point,result) in enumerate(zip(sample["next_points"],sample["next_history"],strict=True)):
        best=None;distance=1.0
        for offset in range(len(saved)):
            candidate=(index+offset)%len(saved)
            ds=[sum((point[f"shape_point_{side}_cm"][a]-saved[candidate][f"shape_point_{side}_cm"][a])**2 for a in "xyz") for side in [0,1]]
            d=ds[0] if sample["geometry"]["kind"] in ["capsule","sphere"] else min(ds)
            if d<.04:best=candidate;break
            if d<distance:distance=d;best=candidate
        assert result["has_static_friction_anchor"]==(best is not None)
        assert result["initial_contact"]==(best is None)
        matched+=best is not None;unmatched+=best is None
        for side in [0,1]:
            wanted=(saved[best] if best is not None else point)[f"shape_point_{side}_cm"]
            if best is None and sample["restored"]:
                wanted={a:points[index][f"shape_point_{side}_cm"][a]+history[f"anchor_offset_{side}_cm"][i] for i,a in enumerate("xyz")}
            actual=result[f"shape_anchor_{side}_cm"]
            assert max(abs(actual[a]-wanted[a]) for a in "xyz")<2e-5,(sample["input"]["label"],index,side,best,actual,wanted)
        assert result["initial_phi_cm"]==0
        assert all(math.isfinite(result["world_anchor_delta_cm"][a]) for a in "xyz")
        world=[]
        for side,geometry,key in [(0,"geometry","body_rotation_xyzw"),(1,"other_geometry","mesh_rotation_xyzw")]:
            local=sample[geometry]["shape_to_body"]["rotation"]
            delta=[result[f"shape_anchor_{side}_cm"][a]-point[f"shape_point_{side}_cm"][a] for a in "xyz"]
            delta=rotated([local[a] for a in "xyzw"],delta)
            delta=rotated(sample["input"]["next"][key],delta)
            world.append([point[f"world_point_{side}_cm"][a]+delta[i] for i,a in enumerate("xyz")])
        assert max(abs(world[0][i]-world[1][i]-result["world_anchor_delta_cm"][a]) for i,a in enumerate("xyz"))<2e-5
assert {4,6,8,14}<=counts and matched>0 and unmatched>0
print(f"physics contact history PASS: {len(samples)} samples, {matched} matched and {unmatched} unmatched points")
