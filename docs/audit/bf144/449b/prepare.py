import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
text=Path('docs/audit/bf144/449a/observer.rs').read_text().replace('C449A','C449B').replace('c449a_','c449b_')
needle='    clips:Res<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>,\n';assert text.count(needle)==1;text=text.replace(needle,'    unit_resources:(Res<C449BUnitFactor>,Res<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>),\n')
needle='    let Some((avatar,parent,instance))=avatars.iter().next() else{return;};';assert text.count(needle)==1;text=text.replace(needle,'    let(factor,clips)=unit_resources;\n'+needle)
needle='"idle_leaf":if before{Vec::new()}else{c449b_idle_leaf(player,&graphs,&clips,&skeletons,&assets)},';assert text.count(needle)==1
text=text.replace(needle,needle+'\n        "unit_factor":serde_json::json!({"applications":factor.applications,"info":factor.info}),\n        "pelvis_markers":if before{Vec::new()}else{c449b_markers(player,&graphs,&skeletons)},')
text+=(r/'factor.rs').read_text();(r/'observer.rs').write_text(text)
old=Path('docs/audit/bf144/449a/playground_observed.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle);a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text();injection=old[a:b].replace('C449A','C449B').replace('c449a_','c449b_')
injection=injection.replace('.after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)','.after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs).after(c449b_unit_factor)')
injection+='    app.init_resource::<C449BUnitFactor>();\n    app.add_systems(PostUpdate,c449b_unit_factor.after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs).before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));\n'
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+text)
args=json.loads(Path('docs/audit/bf144/449a/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/449a/','/449b/').replace('phase449a','phase449b')for v in args],indent=2)+'\n')
for name in ['run_python.ps1','run.ps1','functional.ps1']:(r/name).write_text(Path('docs/audit/bf144/449a',name).read_text().replace('/449a','/449b').replace('ALS_C449A_OUTPUT','ALS_C449B_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/449a/observer.rs',added='Diagnostic cloned Idle clip with only pelvis height unit-factor replacement and actual consumers',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared controlled single-height cloned Idle clip and actual pelvis consumer markers',flush=True)
