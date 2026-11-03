import gzip
import json
from pathlib import Path

root = Path(__file__).resolve().parent
audit = root.parent

def read(path):
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())

def transform(value):
    return {field: [value[field][axis] for axis in axes] for field, axes in [('translation', 'xyz'), ('rotation', 'xyzw'), ('scale', 'xyz')]}

prior = read(audit / '481/inputs.json.gz')
setter = read(audit / '473/inputs.json')['rows']
setters = {(row['dataset'], row['case'] if row['dataset'] == 'weights' else None, row['query'], row['pc']): row for row in setter}
names = prior['names']
source = []
for dataset in ['actual', 'weights']:
    cases = read(audit / f'476/{dataset}-after.native.json.gz')['rig_node_history_probe']['cases']
    for row in prior['source']:
        if row['dataset'] != dataset:
            continue
        case = cases[row['case']]
        pc = row['pc'] + 9
        spec = setters[dataset, case['name'] if dataset == 'weights' else None, row['query'], pc]
        foot = names.index(spec['foot'])
        assert foot == row['indices'][2]
        assert row['expected'][foot]['dirty'] != 'Global'
        assert row['expected'][foot]['global_cache'] == transform(spec['before'])
        entry = dict(dataset=dataset, case=row['case'], query=row['query'], pc=pc, foot=foot,
                     weight=spec['weight'], rotation=spec['value'], before=row['expected'],
                     expected_foot=transform(spec['expected_after']))
        if pc == 130:
            frame = case['frames'][row['query']]
            local = {bone['name']: transform(bone['local_transform']) for bone in frame['after']['bones']}
            global_ = {bone['name']: transform(bone['current_global_transform']) for bone in frame['hierarchy_after']['bones']}
            assert set(local) == set(global_) == set(names)
            entry.update(local=[local[name] for name in names], globals=[global_[name] for name in names])
        source.append(entry)
assert len(source) == 1332
assert sum('local' in row for row in source) == 666
(root / 'inputs.json').write_text(json.dumps(dict(parents=prior['parents'], names=names, tolerance=prior['current_translation_tolerance'], source=source, current=[]), separators=(',', ':'))+'\n')
print('Bound 1332 source setter calls and 666 complete native pose consumers', flush=True)
