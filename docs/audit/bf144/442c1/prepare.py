import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
s = Path("docs/audit/bf144/442/observer.rs").read_text().replace("C442", "C442C1").replace("c442_", "c442c1_")
s = s.replace("camera_rows: Vec<serde_json::Value>,", "camera_rows: Vec<serde_json::Value>,\n    cold_rows: Vec<serde_json::Value>,")
s = s.replace("camera_rows:Vec::new()", "camera_rows:Vec::new(),cold_rows:Vec::new()")
s = s.replace("view.0 = -std::f32::consts::FRAC_PI_2;", "view.0 = -5.0*std::f32::consts::PI/6.0;")
s = s.replace('"rows":probe.rows,', '"rows":probe.rows,"natural_cold":probe.cold_rows,')
s = s.replace("    views: Query<(Entity,&Camera", "    actor_transforms: Query<&Transform,With<bevy_als::als::character::AlsCharacter>>,\n    views: Query<(Entity,&Camera")
s = s.replace('"ordinal": probe.frame,', '"ordinal": probe.frame,"actor_yaw":actor_transforms.get(actor).ok().map(|t|t.rotation.to_euler(EulerRot::YXZ).0),')
s += '''
fn c442c1_camera(
    probe: Res<C442C1Probe>,
    mut cameras: Query<(&mut Camera,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
) {
    let Some(actor)=probe.actor else{return;};
    for (mut camera,owner) in &mut cameras {
        if owner.character==actor { camera.is_active=!(20..22).contains(&probe.frame); }
    }
}
fn c442c1_cold(
    ready: Res<ManualPlaygroundStartupReady>,
    mut probe: ResMut<C442C1Probe>,
    cameras: Query<(&Camera,Ref<GlobalTransform>,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
    actors: Query<(Entity,&bevy_als::als::motion::AlsViewYaw,Option<&bevy_als::als::view::AlsPlayerViewYaw>,
        Option<&bevy_als::als::input::AlsInputCommand>,Option<&bevy_als::als::motion::AlsInputOverride>,
        Has<bevy_als::als::motion::AlsMovementInputBlocked>,Option<&bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>>),With<bevy_als::als::character::AlsCharacter>>,
) {
    if ready.passed{return;}
    for (actor,view,player,command,override_command,blocked,controller) in &actors {
        let effective=command.map(|c|bevy_als::als::motion::movement_input_for_action(*override_command.map_or(c,|v|&v.0),blocked));
        probe.cold_rows.push(serde_json::json!({"entity":format!("{actor:?}"),"raw_yaw":view.0,
            "player_yaw":player.map(|v|v.0),"blocked":blocked,"input":effective.map(|v|v.move_dir.to_array()),
            "desired":controller.map(|v|v.basis.desired_motion.to_array()),
            "cameras":cameras.iter().filter(|(_,_,owner)|owner.character==actor).map(|(camera,transform,_)|serde_json::json!({"active":camera.is_active,"added":transform.is_added(),"yaw":transform.rotation().to_euler(EulerRot::YXZ).0})).collect::<Vec<_>>()}));
    }
}
'''
(r / "observer.rs").write_text(s)
source = Path("src/als_extras/levels/manual_playground.rs").read_text()
needle = "    if let AppExit::Error(code) = app.run() {"
assert source.count(needle) == 1
injection = '''    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0/30.0)));
    app.init_resource::<C442C1Probe>();
    app.add_systems(First,(c442c1_drive,c442c1_camera).chain());
    app.add_systems(Update,(c442c1_cold,c442c1_sample).chain().after(bevy_als::als::dynamics::grounded_dynamics_motor_system).before(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(Last,c442c1_late);
'''
(r / "playground_observed.rs").write_text(source.replace(needle, injection + needle) + s)
args = json.loads(Path("docs/audit/bf144/442/rustc-args.json").read_text())
(r / "rustc-args.json").write_text(json.dumps([v.replace("/442/", "/442c1/").replace("phase442", "phase442c1") for v in args], indent=2) + "\n")
for old, new in (("compile.ps1", "compile.ps1"), ("run.ps1", "run.ps1"), ("run_python.ps1", "run_python.ps1")):
    text = Path("docs/audit/bf144/442", old).read_text().replace("/442", "/442c1").replace("ALS_C442_OUTPUT", "ALS_C442C1_OUTPUT")
    (r / new).write_text(text)
(r / "provenance.json").write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(source.encode()).hexdigest(), original_scene_retained=True, temporary_camera_off_ordinals=[20, 21], natural_cold_is_passive=True, source_camera_cache_injected=False, screenshots=0, performance_acceptance="paused"), indent=2) + "\n")
print("Prepared original Playground lifecycle observation", flush=True)
