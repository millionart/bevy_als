import gzip
import json
from pathlib import Path

root = Path(__file__).resolve().parent
audit = root.parent
def read(name):
    path = audit/name
    if not path.exists():
        path = Path(str(path)+".gz")
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())

print('Reading complete observed history', flush=True)
observed = read('484/runtime-observed/parameters.json')
before = [row.pop('foot_feedback') for row in observed['animation_before']]
after = [row.pop('foot_feedback') for row in observed['animation_after']]
print('Checking old full trajectory', flush=True)
assert observed == read('458/runtime-restored/parameters.json.gz')
assert len(before) == len(after) == 319
assert all(row['valid'] == other['valid'] and row['states'] == other['states'] for row,other in zip(before,after,strict=True))
assert not before[0]['valid']
assert [i for i,row in enumerate(before) if row['became_valid']] == [1]
assert all(row['valid'] and row['use_ik'] and not row['teleport_active'] for row in before[1:])
assert not any(state['base_present'] for row in before for state in row['states'])

controls = read('483/replay-unit-restored.json.gz')['held_root_counterexamples']
correction = {(row['query'],row['bone']):row for row in controls}
definition = read('459/inputs.json')['definition']
names = [bone['name'] for bone in definition['bones']]
parents = [bone['parent'] for bone in definition['bones']]
rows = []
verified_chain_nodes = 0
for query in range(1,319):
    observation = before[query]
    preceding = before[query-1]
    prior_pose = observed['animation_after'][query-1]['rig_snapshot']['poses']['root_result']['pose']
    by_name = {bone['name']:bone for bone in prior_pose['bones']}
    assert set(by_name) == set(names)|{'SKM_Als'}
    assert len(prior_pose['bones']) == len(by_name)
    armature = observation['armature']['chain']
    assert len(armature) == len(preceding['armature']['chain']) == 2
    scene = None
    for name,chain in zip(['ik_foot_l','ik_foot_r','pelvis'],[*observation['targets'],observation['pelvis']],strict=True):
        chain = chain['chain']
        assert chain[0]['bone'] == name and chain[-len(armature):] == armature
        index = names.index(name)
        prefix = 0
        while index is not None:
            node = chain[prefix]
            assert node['bone'] == names[index]
            expected = by_name[names[index]]
            assert all(node[field] == expected[field] for field in ['translation','rotation','scale'])
            verified_chain_nodes += 1
            prefix += 1
            index = parents[index]
        external = chain[prefix:-len(armature)]
        assert all(node['bone'] is None for node in external)
        if scene is not None:
            assert scene == external
        scene = external
    rig = observation['rig']
    consumed = observed['animation_after'][query]['rig_snapshot']['externals']['RigInput']['value']['List']
    assert consumed[1:3] == [{'Bool':True},{'Bool':True}]
    for side,index in [(0,7),(1,9)]:
        assert consumed[index:index+2] == [{'Vector':rig['location'][side]}, {'Quaternion':rig['rotation'][side]}]
    history = observed['animation_before'][query]['history_input']
    assert history['locomotion'] == 'Grounded' and history['stance'] == 'Als.Stance.Standing'
    locals_ = []
    corrections = {}
    for name in names:
        bone = by_name[name]
        locals_.append({field:bone[field] for field in ['translation','rotation','scale']})
        candidate = correction.get((query-1,name))
        if candidate is not None:
            assert bone['rotation'] == candidate['root_f32']
            corrections[name] = candidate['native_end_normalization']
    rows.append(dict(query=query, prior_root=query-1, observation=observation, prior=preceding,
                     locals=locals_, scene=scene, corrections=corrections))
assert len(rows) == 318
(root/'inputs.json').write_text(json.dumps(dict(names=names,parents=parents,rows=rows),separators=(',', ':'))+'\n')
(root/'input-check.json').write_text(json.dumps(dict(gate='PASS_EXACT_OLD_SCENE_PRIOR_ROOT_AND_CONSUMED_RIG_INPUT',
    live_frames=319, valid_frames=318, old_full_trajectory_exact=True,
    selector='ik_foot_l / ik_foot_r', prior_chain_nodes=verified_chain_nodes,
    rig_input_fields=318*4, static_base_and_no_teleport=True),indent=2)+'\n')
print(f'Qualified 318 actual prior Root inputs, {verified_chain_nodes} published chain nodes and all consumed RigInput T/Q fields',flush=True)
