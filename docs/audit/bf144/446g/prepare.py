import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
s=Path('docs/audit/bf144/446e/observer.rs').read_text().replace('C446E','C446G').replace('c446e_','c446g_')
needle='    animation_after: Vec<serde_json::Value>,'
assert s.count(needle)==1
s=s.replace(needle,needle+'\n    physics_motor_rows:Vec<serde_json::Value>,\n    physics_coefficient_rows:Vec<serde_json::Value>,\n    physics_raw_rows:Vec<serde_json::Value>,')
s=s.replace('animation_after:Vec::new()', 'animation_after:Vec::new(),physics_motor_rows:Vec::new(),physics_coefficient_rows:Vec::new(),physics_raw_rows:Vec::new()')
needle='"animation_after":probe.animation_after,'
assert s.count(needle)==1
s=s.replace(needle,needle+'"physics_motors":probe.physics_motor_rows,"physics_coefficients":probe.physics_coefficient_rows,"physics_raw":probe.physics_raw_rows,')
s+=(r/'physics_observer.rs').read_text()
(r/'observer.rs').write_text(s)
old=Path('docs/audit/bf144/446e/playground_observed.rs').read_text()
needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle);a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text()
injection=old[a:b].replace('C446E','C446G').replace('c446e_','c446g_')
injection+='''    app.add_systems(Update,c446g_physics_motor.after(c446g_motor));
    app.add_systems(avian3d::prelude::PhysicsSchedule,c446g_physics_coefficients
        .after(avian3d::dynamics::solver::schedule::SolverSystems::PreSubstep)
        .before(avian3d::dynamics::solver::schedule::SolverSystems::Substep));
    app.add_systems(PostUpdate,c446g_physics_raw
        .after(avian3d::prelude::PhysicsSystems::Last)
        .before(bevy_als::als::motion::AlsActualVelocitySet));
'''
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+s)
args=json.loads(Path('docs/audit/bf144/446e/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/446e/','/446g/').replace('phase446e','phase446g')for v in args],indent=2)+'\n')
for name in ['run_python.ps1','compile.ps1','run.ps1','functional.ps1']:
    (r/name).write_text(Path('docs/audit/bf144/446e',name).read_text().replace('/446e','/446g').replace('ALS_C446E_OUTPUT','ALS_C446G_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/446e/observer.rs',added='Read-only all cold/ready motor, solver coefficients and raw physics keyed by actual application frame',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared cold and ready actual force/coefficient/raw-physics phase observation',flush=True)
