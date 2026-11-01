import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
s = Path('docs/audit/bf144/443/observer.rs').read_text().replace('C443', 'C444').replace('c443_', 'c444_')
s = s.replace('    motor_rows: Vec<serde_json::Value>,', '    motor_rows: Vec<serde_json::Value>,\n    animation_before: Vec<serde_json::Value>,\n    animation_after: Vec<serde_json::Value>,')
s = s.replace('motor_rows:Vec::new()', 'motor_rows:Vec::new(),animation_before:Vec::new(),animation_after:Vec::new()')
s = s.replace('"after_motors":probe.motor_rows,', '"after_motors":probe.motor_rows,"animation_before":probe.animation_before,"animation_after":probe.animation_after,')
s += (r / 'animation_observer.rs').read_text()
(r / 'observer.rs').write_text(s)
original = Path('src/als_extras/levels/manual_playground.rs').read_text()
old = Path('docs/audit/bf144/443/playground_observed.rs').read_text()
needle = '    if let AppExit::Error(code) = app.run() {'
b = old.index(needle)
a = old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration', 0, b)
injection = old[a:b].replace('C443', 'C444').replace('c443_', 'c444_')
injection += '''    app.world_mut().resource_mut::<RenderEffectsProbe>().directory=None;
    app.add_systems(PostUpdate,(|| true).pipe(c444_animation)
        .after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
    app.add_systems(PostUpdate,(|| false).pipe(c444_animation)
        .after(bevy_animation_graph::core::plugin::AnimationGraphSet::Final));
'''
assert original.count(needle) == 1
(r / 'playground_observed.rs').write_text(original.replace(needle, injection + needle) + s)
args = json.loads(Path('docs/audit/bf144/443/rustc-args.json').read_text())
(r / 'rustc-args.json').write_text(json.dumps([v.replace('/443/', '/444/').replace('phase443', 'phase444') for v in args], indent=2) + '\n')
for name in ['compile.ps1', 'run.ps1', 'functional.ps1']:
    (r / name).write_text(Path('docs/audit/bf144/443', name).read_text().replace('/443', '/444').replace('ALS_C443_OUTPUT', 'ALS_C444_OUTPUT'))
(r / 'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(), base_observer='docs/audit/bf144/443/observer.rs', added='Read-only actual motion, provider inputs and player clock before and after evaluation', production_changed=False, render_gpu_pass_acceptance='deferred under existing explicit peripheral exclusion', screenshots=0, performance_acceptance='paused'), indent=2) + '\n')
print('Prepared current motion to animation input and clock observation', flush=True)
