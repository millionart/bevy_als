import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
text=Path('docs/audit/bf144/447b/observer.rs').read_text().replace('C447B','C448B').replace('c447b_','c448b_')
needle='"head_values":c448b_head_values(player,&graphs),';assert text.count(needle)==1
text=text.replace(needle,needle+'\n        "pose_boundaries":c448b_pose_boundaries(player,&graphs,&skeletons,&assets),')
needle='    graphs:Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,\n    assets:Res<AssetServer>,';assert text.count(needle)==1
text=text.replace(needle,'    skeletons:Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,\n'+needle)
text+=(r/'pose_observer.rs').read_text();(r/'observer.rs').write_text(text)
old=Path('docs/audit/bf144/447b/playground_observed.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle);a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text();injection=old[a:b].replace('C447B','C448B').replace('c447b_','c448b_')
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+text)
args=json.loads(Path('docs/audit/bf144/447b/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/447b/','/448b/').replace('phase447b','phase448b')for v in args],indent=2)+'\n')
for name in ['run_python.ps1','run.ps1','functional.ps1']:(r/name).write_text(Path('docs/audit/bf144/447b',name).read_text().replace('/447b','/448b').replace('ALS_C447B_OUTPUT','ALS_C448B_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/447b/observer.rs',added='Read-only actual Root and five updated Pose caches with context curves',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared actual five Pose cache boundaries and default Root output without changing input or state',flush=True)
