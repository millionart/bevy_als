import hashlib
import json
import gzip
from pathlib import Path

r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text())
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
units=[('src/control_rig/control_rig_query.rs',Path('src/control_rig/control_rig_query.rs'),[(10,59)]),('src/als/character/control_rig_apply.rs',Path('src/als/character/control_rig_apply.rs'),[(423,446)]),('src/als_extras/levels/playground.rs',Path('src/als_extras/levels/playground.rs'),[(65,105),(268,283),(570,609)]),('assets/ALSExtras/Levels/L_Als_Playground/playground_scene.ron',Path('assets/ALSExtras/Levels/L_Als_Playground/playground_scene.ron'),[(23,23),(61,61)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'),[(5150,5180),(5217,5223),(5240,5295)]),('source:Private/Nodes/AlsRigUnit_FootOffsetTrace.cpp',plugin/'Source/ALS/Private/Nodes/AlsRigUnit_FootOffsetTrace.cpp',[(8,61)])]
facts=[]
for key,p,ranges in units:
 raw=p.read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual floor origin and bounds, public query arithmetic, source world binding and continuous input stages',flush=True)
v=json.loads((r/'floor.native.json').read_text()if (r/'floor.native.json').exists()else gzip.decompress((r/'floor.native.json.gz').read_bytes()))
p={x['name']:x for x in v['asset_properties']};bounds={x['name']:{y['name']:y['numeric_value']for y in x.get('fields',[])}for x in p['ExtendedBounds']['fields']}
assert bounds['Origin']==dict(X=0,Y=0,Z=-25)and bounds['BoxExtent']==dict(X=1000,Y=1000,Z=25)
points=v['static_mesh_collision']['vertices_source'];lower=[min(x[k]for x in points)for k in 'xyz'];upper=[max(x[k]for x in points)for k in 'xyz'];assert lower==[-1000,-1000,-50]and upper==[1000,1000,0]
body=next(x for x in v['asset_objects']if x['class_path']=='/Script/Engine.BodySetup');properties={x['name']:x for x in body['properties']};box=next(x for x in properties['AggGeom']['fields']if x['name']=='BoxElems')['exported_text'];assert box=='((Center=(X=0.000000,Y=0.000000,Z=-25.000000),X=2000.000000,Y=2000.000000,Z=50.000000,bIsGenerated=True))'
(r/'floor-check.json').write_text(json.dumps(dict(gate='PASS_FRESH_FLOOR_LOCAL_BOUNDS_COLLISION_BOX_AND_MESH_EXTENTS',source_origin_cm=[0,0,-25],source_half_extents_cm=[1000,1000,25],collision_lower_cm=lower,collision_upper_cm=upper,body_box=box,source_collision_trace_flag=properties['CollisionTraceFlag']['exported_text']),indent=2)+'\n')
print('Verified fresh floor source record origin, extent and physical surface',flush=True)
