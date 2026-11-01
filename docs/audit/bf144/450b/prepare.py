import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
text=Path('docs/audit/bf144/450a/observer.rs').read_text().replace('C450A','C450B').replace('c450a_','c450b_')
text=text[:text.index('fn c450b_difference(')]+(r/'difference_observer.rs').read_text()
needle='    skeletons:Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,\n';assert text.count(needle)==1;text=text.replace(needle,'    factor:Res<C450BDataFactor>,\n'+needle)
needle='"forward_difference":if before{Vec::new()}else{c450b_difference(player,&graphs,&skeletons,&assets)},';assert text.count(needle)==1
text=text.replace(needle,needle+'\n        "data_factor":serde_json::json!({"applications":factor.applications,"info":factor.info}),\n        "pelvis_markers":if before{Vec::new()}else{c450b_markers(player,&graphs,&skeletons)},')
text+=(r/'factor.rs').read_text();(r/'observer.rs').write_text(text)
old=Path('docs/audit/bf144/450a/playground_observed.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle);a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text();injection=old[a:b].replace('C450A','C450B').replace('c450a_','c450b_')
injection=injection.replace('.after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)','.after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs).after(c450b_data_factor)')
injection+='    app.init_resource::<C450BDataFactor>();\n    app.add_systems(PostUpdate,c450b_data_factor.after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs).before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));\n'
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+text)
args=json.loads(Path('docs/audit/bf144/450a/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/450a/','/450b/').replace('phase450a','phase450b')for v in args],indent=2)+'\n')
for name in ['run_python.ps1','run.ps1','functional.ps1']:(r/name).write_text(Path('docs/audit/bf144/450a',name).read_text().replace('/450a','/450b').replace('ALS_C450A_OUTPUT','ALS_C450B_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/450a/observer.rs',added='Only Forward pelvis half-key X data control and actual Root markers',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared actual Forward difference inputs and full public replay without input substitution',flush=True)
