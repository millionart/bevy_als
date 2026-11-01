import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
s = Path("docs/audit/bf144/442/observer.rs").read_text().replace("C442", "C443").replace("c442_", "c443_")
s = s.replace("camera_rows: Vec<serde_json::Value>,", "camera_rows: Vec<serde_json::Value>,\n    motor_rows: Vec<serde_json::Value>,")
s = s.replace("camera_rows:Vec::new()", "camera_rows:Vec::new(),motor_rows:Vec::new()")
late = "fn c443_late(\n    mut probe: ResMut<C443Probe>,"
assert s.count(late) == 1
s = s.replace(late, late + '''
    bodies: Query<(&Position,&LinearVelocity),With<bevy_als::als::character::AlsCharacter>>,
    physics: Res<Time<avian3d::prelude::Physics>>,
    substep_time: Res<Time<avian3d::prelude::Substeps>>,
    substeps: Res<avian3d::prelude::SubstepCount>,
    gravity: Res<avian3d::prelude::Gravity>,''')
s = s.replace('let ordinal=probe.frame-1;probe.camera_rows.push(serde_json::json!({"ordinal":ordinal,"camera":camera}));', '''let ordinal=probe.frame-1;
    let Ok((position,velocity))=bodies.get(actor) else{panic!("Observed body is missing");};
    probe.camera_rows.push(serde_json::json!({"ordinal":ordinal,"camera":camera,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"physics_delta":physics.delta_secs(),"substep_delta":substep_time.delta_secs(),"substeps":substeps.0,"gravity":gravity.0.to_array()}));''')
s = s.replace('"rows":probe.rows,', '"rows":probe.rows,"after_motors":probe.motor_rows,')
s = s.replace('"planned_delta": cmc.grounded_movement_delta.map(|v|v.to_array()),', '"planned_delta": cmc.grounded_movement_delta.map(|v|v.to_array()),"grounded_move_start":cmc.grounded_move_start.map(|v|v.to_array()),')
s = s.replace('bodies: Query<(&Position,&LinearVelocity),With<bevy_als::als::character::AlsCharacter>>,\n    physics:', 'bodies: Query<(&Position,&LinearVelocity,&avian3d::prelude::ComputedCenterOfMass),With<bevy_als::als::character::AlsCharacter>>,\n    frame_time: Res<Time<Virtual>>,\n    physics:')
s = s.replace('let Ok((position,velocity))=bodies.get(actor) else{panic!("Observed body is missing");};', 'let Ok((position,velocity,com))=bodies.get(actor) else{panic!("Observed body is missing");};')
s = s.replace('"physics_delta":physics.delta_secs(),', '"physics_delta":physics.delta_secs(),"physics_delta_ns":physics.delta().as_nanos() as u64,"frame_delta_ns":frame_time.delta().as_nanos() as u64,"substep_delta_ns":substep_time.delta().as_nanos() as u64,"center_of_mass":com.0.to_array(),')
s += '''
fn c443_motor(
    mut probe: ResMut<C443Probe>,
    bodies: Query<(&Position,&LinearVelocity),With<bevy_als::als::character::AlsCharacter>>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let Ok((position,velocity))=bodies.get(actor) else{panic!("Observed motor body is missing");};
    let ordinal=probe.frame-1;
    probe.motor_rows.push(serde_json::json!({"ordinal":ordinal,"position":position.0.to_array(),"velocity":velocity.0.to_array()}));
}
'''
(r / "observer.rs").write_text(s)
source = Path("src/als_extras/levels/manual_playground.rs").read_text()
needle = "    if let AppExit::Error(code) = app.run() {"
assert source.count(needle) == 1
injection = '''    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0/30.0)));
    app.init_resource::<C443Probe>();
    app.add_systems(First,c443_drive);
    app.add_systems(Update,c443_sample.after(bevy_als::als::dynamics::grounded_dynamics_motor_system).before(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(Update,c443_motor.after(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(Last,c443_late);
'''
(r / "playground_observed.rs").write_text(source.replace(needle, injection + needle) + s)
args = json.loads(Path("docs/audit/bf144/442/rustc-args.json").read_text())
(r / "rustc-args.json").write_text(json.dumps([v.replace("/442/", "/443/").replace("phase442", "phase443") for v in args], indent=2) + "\n")
for name in ("compile.ps1", "run.ps1", "run_python.ps1"):
    text = Path("docs/audit/bf144/442", name).read_text().replace("/442", "/443").replace("ALS_C442_OUTPUT", "ALS_C443_OUTPUT")
    (r / name).write_text(text)
(r / "provenance.json").write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(source.encode()).hexdigest(), baseline_observer="docs/audit/bf144/442/observer.rs", readonly_added=["after Tnua Motors body position and velocity", "Last body position and velocity", "actual physics and substep clocks, count, gravity"], source_data_injected=False, original_scene_retained=True, screenshots=0, performance_acceptance="paused"), indent=2) + "\n")
print("Prepared original Playground with readonly motor and post-physics stages", flush=True)
