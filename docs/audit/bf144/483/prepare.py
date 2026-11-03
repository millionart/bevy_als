import gzip
import json
from pathlib import Path

root = Path(__file__).resolve().parent
audit = root.parent

def read(name):
    path = audit / name
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())

def transforms(bones, field):
    return {bone['name']: {key:[bone[field][key][axis] for axis in axes] for key,axes in
                           [('translation','xyz'),('rotation','xyzw'),('scale','xyz')]} for bone in bones}

print('Reading source character and publication boundaries', flush=True)
case = read('457/restored-final.native.json.gz')['character_animation_probe']['cases'][0]
source = []
for query, frame in enumerate([case['initialization'], *case['frames']]):
    packet = frame['pose_boundaries']
    before = packet['rig_source']
    rig = packet['rig_output']
    output = packet['root_result']
    assert before['engine_frame'] == rig['engine_frame'] == output['engine_frame']
    assert before['required_bones_asset'] == rig['required_bones_asset'] == output['required_bones_asset']
    assert before['attribute_count'] == rig['attribute_count'] == output['attribute_count'] == 0
    assert rig['pose'] == output['pose']
    assert before['pose']['curves'] == rig['pose']['curves'] == frame['curves']
    mesh = transforms(frame['mesh_local_pose'], 'local')
    pose = transforms(output['pose']['bones'], 'local_transform')
    assert len(pose) == len(mesh) == 79 and mesh.keys() == pose.keys()
    feedback = None
    if query:
        main = next(item for item in frame['instances_before_tick'] if item['main'])
        prior = source[-1]
        feedback = main['proxy_attribute_curves']
        assert feedback == prior['curves']
        assert main['proxy_native_update_engine_frame'] == prior['engine_frame']
        assert output['engine_frame'] == prior['engine_frame'] + 1
    source.append(dict(query=query, engine_frame=output['engine_frame'],
                       curves=output['pose']['curves'], feedback=feedback, pose=pose, mesh=mesh))
assert len(source) == 319
del case

print('Joining the independent VM results with actual public Root packets', flush=True)
vm = read('482/replay-unit-restored.json.gz')
definition = read('459/inputs.json')['definition']
names = [bone['name'] for bone in definition['bones']]
computed = {row['query']:dict(zip(names,row['result']['local'],strict=True))
            for row in vm['rows'] if row['domain'] == 'current' and row['mode'] == 0 and row['pc'] == 130}
del vm
zero = read('459/replay-restored.json.gz')['outputs'][0]
assert zero['query'] == 0
computed[0] = {bone['name']:bone['local'] for bone in zero['local']}
assert set(computed) == set(range(319))
actual = read('458/runtime-restored/parameters.json.gz')
target = []
for query, frame in enumerate(actual['animation_after']):
    snapshot = frame['rig_snapshot']
    r = snapshot['poses']
    ragdoll = frame['ragdoll_blend'][0]
    assert ragdoll['actual_state']['weights'] == [1,0] and ragdoll['actual_state']['last_active_index'] == 0
    assert r['rig_output']['pose'] == ragdoll['base']
    assert r['root_result']['pose'] == ragdoll['output']
    assert all(set(p['pose']) == {'bones','timestamp','skeleton_asset','root_motion'} for p in r.values())
    assert all(p['pose']['root_motion'] is None for p in r.values())
    envelope = lambda p: {key:p[key] for key in ['timestamp','skeleton_asset','root_motion']}
    assert envelope(r['rig_source']['pose']) == envelope(r['rig_output']['pose']) == envelope(r['root_result']['pose'])
    target.append(dict(query=query, input=r['rig_source']['pose'], rig=r['rig_output']['pose'],
                       root=r['root_result']['pose'], solved=computed[query],
                       curves=snapshot['upstream_context_curves'],
                       feedback=actual['animation_before'][query]['curve_frame_input'],
                       ragdoll_weight=ragdoll['actual_state']['weights'][0]))
assert len(target) == 319
(root/'inputs.json').write_text(json.dumps(dict(source=source,target=target),separators=(',', ':'))+'\n')
print('Qualified independent target inputs, both source Root edges and 318 native prior-curve reads', flush=True)
