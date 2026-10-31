import json
import re
from pathlib import Path
r=Path(__file__).resolve().parent
v=json.loads((r/'curves.native.json').read_text())['blend_profile_probe']['samples']
s=Path('src/als/settings/params.rs').read_text();a=s.index('impl HermiteCurve {');b=s.index('\n#[derive',a);method=s[a:b]
normal=s[s.index('normal: source_dynamics_curve('):s.index('responsive: source_dynamics_curve(')]
tuples=re.findall(r'\(([-0-9._]+), ([-0-9._]+), ([-0-9._]+), ([-0-9._]+)\)',normal);assert len(tuples)==12
keys=[]
for axis in range(3):
    keys.append('HermiteCurve { keys: vec!['+','.join('HermiteKey { time:'+a+', value:'+b+', arrive_tangent:'+c+', leave_tangent:'+d+' }'for a,b,c,d in tuples[axis*4:axis*4+4])+'] }')
times=[x['time']for x in v[0]['custom_curve_time_samples']];assert all([x['time']for x in a['custom_curve_time_samples']]==times for a in v)
tangent=v[0]['custom_curve_keys'][2]['leave_tangent']
text='''#[derive(Clone,Copy)]
pub struct HermiteKey {pub time:f32,pub value:f32,pub arrive_tangent:f32,pub leave_tangent:f32}
#[derive(Clone)]
pub struct HermiteCurve {pub keys:Vec<HermiteKey>}
'''+method+'''
fn main(){let curves=['''+','.join(keys)+'''];let mut corrected=curves[0].clone();corrected.keys[2].leave_tangent='''+repr(tangent)+'''f32/100.0;let times=['''+','.join(repr(float(x))+'f32'for x in times)+'''];let mut output=Vec::new();for t in times{for c in &curves{output.extend(c.sample(t).to_le_bytes());}output.extend(corrected.sample(t).to_le_bytes());}std::fs::write(std::env::args().nth(1).unwrap(),output).unwrap();println!("Captured unchanged target Hermite method excerpt and source-key-only control");}
'''
(r/'curve_excerpt.rs').write_text(text);(r/'method-excerpt.txt').write_text(method);print('Extracted unchanged target method text and all12 current Normal keys;27 native times',flush=True)
