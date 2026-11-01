import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
s=Path('docs/audit/bf144/446/observer.rs').read_text().replace('C446','C446E').replace('c446_','c446e_')
needle='    In(before): In<bool>,'
assert s.count(needle)==1
s=s.replace(needle,needle+'\n    app_frame:Res<bevy::diagnostic::FrameCount>,\n    root_contexts:Query<&bevy_als::animation_context_projection::AlsRootAnimationContext>,')
needle='    let row=serde_json::json!({\n        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),'
assert s.count(needle)==1
extra='''    let root_context=root_contexts.get(instance.player_entity()).ok();
    let curve_frame_input=serde_json::json!({"app_frame":app_frame.0,"virtual_elapsed_ns":virtual_time.elapsed().as_nanos() as u64,
        "projected_context_present":root_context.is_some(),"projected_curve_count":root_context.map(|v|v.curves.len()),
        "projected_standing":root_context.and_then(|v|v.f32("PoseStanding")),"projected_grounded":root_context.and_then(|v|v.f32("PoseGrounded")),
        "global_standing":scalar("pose_state.standing_amount"),"global_grounded":scalar("pose_state.grounded_amount"),
        "source_property_standing":bevy_als::extensions::source_instance::source_player_property(player,"pose_state.standing_amount").and_then(|v|v.as_f32().ok()),
        "source_property_grounded":bevy_als::extensions::source_instance::source_player_property(player,"pose_state.grounded_amount").and_then(|v|v.as_f32().ok())});
'''
s=s.replace(needle,extra+needle+'\n        "curve_frame_input":curve_frame_input,')
(r/'observer.rs').write_text(s)
old=Path('docs/audit/bf144/446/playground_observed.rs').read_text()
needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle)
a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text()
injection=old[a:b].replace('C446','C446E').replace('c446_','c446e_')
needle='.after(bevy_animation_graph::core::plugin::AnimationGraphSet::Final));'
assert injection.count(needle)==1
injection=injection.replace(needle,'.after(bevy_als::animation_context_projection::AlsRootAnimationContextSet));')
(r/'playground_observed.rs').write_text(original.replace('    if let AppExit::Error(code) = app.run() {',injection+'    if let AppExit::Error(code) = app.run() {')+s)
args=json.loads(Path('docs/audit/bf144/446/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/446/','/446e/').replace('phase446','phase446e')for v in args],indent=2)+'\n')
for name in ['compile.ps1','run.ps1','functional.ps1','run_python.ps1']:
    (r/name).write_text(Path('docs/audit/bf144/446',name).read_text().replace('/446','/446e').replace('ALS_C446_OUTPUT','ALS_C446E_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/446/observer.rs',added='Actual application frame and prior curve/provider/property values; after sample ordered after projection',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared actual target prior curve inputs and application frame observation',flush=True)
