import json
import math
import sys
from pathlib import Path

data=json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
asset=data['physics_asset'];settings=asset['constraint_console_variables']
assert asset['project_initial_overlap_depenetration_velocity_cm_s']==-1
assert settings['p.Chaos.Collision.Manifold.EnableGjkWarmStart']=='true'
assert settings['p.Chaos.Collision.UseGJK2']=='false'
samples={s['input']['label']:s for s in data['physics_contact_probe']['samples']}
for label in ['box_deep','box_shallow','capsule_shallow','mesh_shallow']:
    warm=samples[label];cold=samples[label+'_current'];index=warm['input']['mesh_shape_index']
    assert not warm['restored']
    group=next(c for c in cold['aggregate_contacts'] if c['mesh_shape_index']==index)
    assert warm['next_points']==group['points']
    assert warm['fresh_points']==warm['next_points']
    assert 'cold_points' not in warm
    simplex=warm['next_gjk_simplex']
    if label.startswith('box'):
        assert warm['gjk_simplex'] and simplex
        assert math.isclose(sum(v['weight'] for v in simplex),1,abs_tol=1e-9)
        delta=[sum(v['weight']*v['world_delta_cm'][a] for v in simplex) for a in 'xyz']
        distance=math.sqrt(sum(x*x for x in delta))
        if label=='box_deep':
            assert len(simplex)==4 and min(v['weight'] for v in simplex)>0
            assert distance<float(settings['p.Chaos.Collision.GJKEpsilon'])
        else:assert distance>float(settings['p.Chaos.Collision.GJKEpsilon'])
    else:assert not warm['gjk_simplex'] and not simplex
print('GJK history PASS: independent cold constraints, separated core, EPA and uncached paths')
