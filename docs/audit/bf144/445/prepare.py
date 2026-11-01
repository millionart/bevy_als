import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
s=Path('docs/audit/bf144/444/observer.rs').read_text().replace('C444','C445').replace('c444_','c445_')
s=s.replace('    In(before): In<bool>,','    In(before): In<bool>,\n    graphs:Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,\n    assets:Res<AssetServer>,')
needle='"ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),'
assert s.count(needle)==1
s=s.replace(needle,needle+'\n        "leaf_nodes":if before{Vec::new()}else{c445_leaf_rows(player,&graphs,&assets)},')
s+=(r/'keys.rs').read_text()+(r/'leaf_observer.rs').read_text()
(r/'observer.rs').write_text(s)
old=Path('docs/audit/bf144/444/playground_observed.rs').read_text()
needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle)
a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text()
injection=old[a:b].replace('C444','C445').replace('c444_','c445_')
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+s)
args=json.loads(Path('docs/audit/bf144/444/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/444/','/445/').replace('phase444','phase445')for v in args],indent=2)+'\n')
for name in ['compile.ps1','run.ps1','functional.ps1']:
    (r/name).write_text(Path('docs/audit/bf144/444',name).read_text().replace('/444','/445').replace('ALS_C444_OUTPUT','ALS_C445_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/444/observer.rs',added='Read-only all leaf declarations and current Default/Temporary clocks after Final',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared complete current leaf clock observation using public reflection and contexts',flush=True)
