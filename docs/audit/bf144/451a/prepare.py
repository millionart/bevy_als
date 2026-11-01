import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
text=Path('docs/audit/bf144/447b/observer.rs').read_text().replace('C447B','C451A').replace('c447b_','c451a_')
needle='"head_values":c451a_head_values(player,&graphs),';assert text.count(needle)==1;text=text.replace(needle,needle+'\n        "head_apply":if before{Vec::new()}else{c451a_head_apply(player,&graphs,&skeletons,&assets)},')
needle='    graphs:Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,\n    assets:Res<AssetServer>,';assert text.count(needle)==1;text=text.replace(needle,'    skeletons:Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,\n'+needle)
pose=Path('docs/audit/bf144/448b/pose_observer.rs').read_text().split('fn c448b_pose_boundaries(')[0].replace('c448b_','c451a_')
text+=pose+(r/'head_observer.rs').read_text();(r/'observer.rs').write_text(text)
old=Path('docs/audit/bf144/447b/playground_observed.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle);a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text();injection=old[a:b].replace('C447B','C451A').replace('c447b_','c451a_')
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+text)
args=json.loads(Path('docs/audit/bf144/447b/rustc-args.json').read_text());(r/'rustc-args.json').write_text(json.dumps([v.replace('/447b/','/451a/').replace('phase447b','phase451a')for v in args],indent=2)+'\n')
for name in ['run_python.ps1','run.ps1','functional.ps1']:(r/name).write_text(Path('docs/audit/bf144/447b',name).read_text().replace('/447b','/451a').replace('ALS_C447B_OUTPUT','ALS_C451A_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/447b/observer.rs',added='Actual full Head Apply inputs, alpha resolution, skeleton defaults and public kernel',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared full Head inputs, resolved alpha, actual skeleton and public operator replay',flush=True)
