import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
scratch = Path('tools/tmp/bf144/579')
scratch.mkdir(parents=True, exist_ok=True)
relative = 'parry3d-0.27.0/src/query/gjk/gjk.rs'
locked = next((Path.home() / '.cargo/registry/src').glob('*/'+relative))
text = locked.read_text()
a = text.index('fn minkowski_ray_cast<')
b = text.index('\nfn result(', a)
kernel = text[a:b].strip()
replacements = (
    ('fn minkowski_ray_cast<', 'fn traced_ray_cast<'),
    ('    simplex: &mut VoronoiSimplex,', '    simplex: &mut VoronoiSimplex,\n    trace: &mut Vec<Value>,\n    omit_bound_return: bool,'),
    ('        let (normalized, dist) = neg_proj_coords.normalize_and_length();',
     '''        let (normalized, dist) = neg_proj_coords.normalize_and_length();
        trace.push(json!({"event":"projection","iteration":niter,"distance":dist,
            "old_bound":old_max_bound,"toi":ltoi/ray_length,"simplex":simplex.dimension(),
            "tolerance":_eps_tol}));'''),
    ('''        } else {
            return Some((ltoi / ray_length, ldir));
        }

        let support_point''',
     '''        } else {
            trace.push(json!({"event":"distance_tolerance","iteration":niter,"toi":ltoi/ray_length}));
            return Some((ltoi / ray_length, ldir));
        }

        let support_point'''),
    ('''        if last_chance && ltoi > 0.0 {
            // last_chance && ltoi > 0.0 && (support_point.point - curr_ray.origin).dot(ldir) >= 0.0 {
            return Some((ltoi / ray_length, ldir));
        }''',
     '''        if last_chance && ltoi > 0.0 {
            trace.push(json!({"event":"non_decreasing_upper_bound","iteration":niter,"toi":ltoi/ray_length,
                "bound":max_bound,"previous":old_max_bound}));
            if !omit_bound_return {
                return Some((ltoi / ray_length, ldir));
            }
        }'''),
    ('''        if last_chance {
            return None;
        }''',
     '''        if last_chance {
            trace.push(json!({"event":"last_chance_miss","iteration":niter,"toi":ltoi/ray_length}));
            return None;
        }'''),
    ('''                    ltoi += t;

                    // NOTE''',
     '''                    ltoi += t;
                    trace.push(json!({"event":"advance","iteration":niter,"toi":ltoi/ray_length}));

                    // NOTE'''),
    ('''        let _ = simplex.add_point(support_point.translate(-curr_ray.origin));
        proj = simplex.project_origin_and_reduce();''',
     '''        let dimension = simplex.dimension();
        let added = simplex.add_point(support_point.translate(-curr_ray.origin));
        proj = simplex.project_origin_and_reduce();
        trace.push(json!({"event":"simplex_update","iteration":niter,"added":added,
            "before":dimension,"after":simplex.dimension(),"projection":proj.to_array()}));'''),
    ('''                return Some((ltoi / ray_length, ldir)); // Vector inside of the cso.''',
     '''                trace.push(json!({"event":"inside_simplex","iteration":niter,"toi":ltoi/ray_length}));
                return Some((ltoi / ray_length, ldir));'''),
)
for old,new in replacements:
    assert kernel.count(old)==1,old
    kernel=kernel.replace(old,new)
kernel='\n'.join(line for line in kernel.splitlines() if not line.lstrip().startswith('//'))
(scratch/'gjk_probe.rs').write_text((root/'probe.rs').read_text()+'\n'+kernel+'\n')
(root/'kernel-source.json').write_text(json.dumps(dict(path=relative,bytes=locked.stat().st_size,
    sha256=hashlib.sha256(locked.read_bytes()).hexdigest()),indent=2)+'\n')

cases=[]
for label in ('positive','negative'):
    live=json.loads((root.parent/'577'/f'runtime-{label}/landing-geometry.json').read_text())
    for stage in ('actual','direction_control','height_control'):
        row=live[stage]
        cases.append(dict(name=f'landing-{label}-{stage}',start=row['start'],direction=row['direction'],
            expected=row['distance'],box_center=live['floor_center'],box_extents=live['floor_half_extents'],
            orientation=live['orientation'],radius=.3,half_height=.9,length=live['max_distance']))
row=json.loads((root.parent/'578'/'runtime-baseline'/'brake-replay.json').read_text())['steps'][0]
def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]


for shape in ('full','short'):
    half=.9 if shape=='full' else f32(f32(.9)-row['short_shrink'])
    cases.append(dict(name=f'brake-{shape}',start=row[f'{shape}_start'],direction=[0,-1,0],
        expected=row[f'{shape}_hit'],box_center=row['box_center'],box_extents=row['box_extents'],
        orientation=[0,0,0,1],radius=.3 if shape=='full' else f32(30*f32(.01)),
        half_height=half,length=row[f'{shape}_limit']))
(root/'inputs.json').write_text(json.dumps(cases,indent=2)+'\n')
args=['--edition=2024','--crate-name','bf144_phase579',str(scratch/'gjk_probe.rs'),'-C','opt-level=1','-L',
      'dependency=target_game/play/deps','-o',str(scratch/'playground_observed')]
for crate in ('parry3d','approx','serde_json','num_traits'):
    library,=Path('target_game/play/deps').glob(f'lib{crate}-*.rlib')
    args.extend(('--extern',f'{crate}={library}'))
(root/'observer-rustc-args.json').write_text(json.dumps(args,indent=2)+'\n')
print('Prepared instrumented locked GJK kernel and eight archived actual inputs',flush=True)
