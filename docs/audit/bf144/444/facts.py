import gzip
import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
config = json.loads(Path('tools/env.local.json').read_text())
plugin = Path(config['ue_project']).parent / 'Plugins/ALS-Refactored'
units = []
for name, ranges in [('AlsCharacter.cpp',[(266,300),(1372,1438)]),('AlsAnimationInstance.cpp',[(98,169),(681,719)])]:
    units.append((plugin/'Source/ALS/Private'/name, 'source:ALS/Private/'+name, ranges))
for name, ranges in [('character/animation_instances/actual_motion.rs',[(26,43),(49,64),(91,140)]),('character/animation_instances/ab_als_provider.rs',[(70,86),(301,339),(403,422)]),('character/character_runtime.rs',[(66,104)]),('character/movement/motion.rs',[(2571,2583),(2673,2694)])]:
    units.append((Path('src/als')/name, 'src/als/'+name, ranges))
units.append((Path('src/animation_graph_ext/post_physics_animation.rs'),'src/animation_graph_ext/post_physics_animation.rs',[(44,60),(182,212)]))
dep = next(Path('target/play/deps').glob('bevy_animation_graph_core-*.d')).read_text().split()
for name,ranges in [('animation_graph_player.rs',[(193,235),(339,351)]),('context/graph_context_arena.rs',[(56,66)]),('systems.rs',[(171,202)])]:
    p=Path(next(v for v in dep if v.endswith('/'+name)))
    units.append((p,'dependency:bevy_animation_graph/'+name,ranges))
facts=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
raw=gzip.decompress(Path('docs/audit/bf144/443/source.native.json.gz').read_bytes())
case=json.loads(raw)['character_animation_probe']['cases'][0]
keys=['frame','world_seconds','world_delta_seconds','pose_ticked_before_world','update_counter','evaluation_counter','movement_before_tick','movement_after_tick','instances_before_tick','linked_instances','source_state_after_tick']
result=dict(native_report='docs/audit/bf144/443/source.native.json.gz',uncompressed_sha256=hashlib.sha256(raw).hexdigest(),initialization=case['initialization'],frames=[{k:f[k]for k in keys}for f in case['frames']])
projection=(json.dumps(result,indent=2)+'\n').encode()
packed=gzip.compress(projection,mtime=0)
assert gzip.decompress(packed)==projection
(r/'source-animation-phases.json.gz').write_bytes(packed)
(r/'projection-compression.json').write_text(json.dumps(dict(uncompressed_bytes=len(projection),uncompressed_sha256=hashlib.sha256(projection).hexdigest(),compressed_bytes=len(packed),compressed_sha256=hashlib.sha256(packed).hexdigest(),round_trip_byte_exact=True),indent=2)+'\n')
print('Captured fresh motion, provider and clock contracts plus native phase projection',flush=True)
