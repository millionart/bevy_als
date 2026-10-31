import hashlib
import json
from pathlib import Path
r=Path(__file__).resolve().parent
old=Path('docs/audit/bf144/441c/observer.rs').read_text();s=old.replace('C441','C442').replace('c441_','c442_').replace('ALS_C441_OUTPUT','ALS_C442_OUTPUT')
s=s.replace('progress: Option<(bool, usize, usize)>,','progress: Option<(bool, usize, usize)>,\n    camera_rows: Vec<serde_json::Value>,').replace('progress: None }','progress: None, camera_rows:Vec::new() }')
s=s.replace('&bevy_als::als::gait::AlsMaxAllowedGait,','(&bevy_als::als::gait::AlsMaxAllowedGait, Option<&bevy_als::als::view::AlsPlayerViewYaw>, Option<&bevy_als::als::motion::AlsInputOverride>, &bevy_als::als::input::AlsInputCommand, Has<bevy_als::als::motion::AlsMovementInputBlocked>),')
s=s.replace('physical, braking, allowed))','physical, braking, (allowed, player_view, override_command, command, input_blocked)))')
s=s.replace('    mut exit: MessageWriter<AppExit>,','    views: Query<(Entity,&Camera,Ref<GlobalTransform>,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,')
needle='    let row = serde_json::json!({'
extra='''    let effective = bevy_als::als::motion::movement_input_for_action(*override_command.map_or(command, |v|&v.0), input_blocked);
    let used_yaw = player_view.map_or(view.0, |v|v.0);
    let expected_direction = bevy_als::als::motion::view_relative_move_direction(effective.move_dir, used_yaw)*effective.move_dir.length().min(1.0);
    let raw_direction = bevy_als::als::motion::view_relative_move_direction(effective.move_dir, view.0)*effective.move_dir.length().min(1.0);
    let camera = views.iter().find(|(_,cam,t,owner)|owner.character==actor && cam.is_active && !t.is_added()).map(|(id,_,t,_)|serde_json::json!({"entity":format!("{id:?}"),"yaw":t.rotation().to_euler(EulerRot::YXZ).0,"rotation":t.rotation().to_array()}));
''';assert s.count(needle)==1;s=s.replace(needle,extra+needle)
s=s.replace('"ordinal": probe.frame,','"ordinal": probe.frame, "player_view_yaw":player_view.map(|v|v.0), "consumed_yaw":used_yaw, "input_move":effective.move_dir.to_array(), "input_blocked":input_blocked, "camera_at_update":camera, "expected_direction":expected_direction.to_array(), "raw_control_direction":raw_direction.to_array(),')
i=s.index('    if probe.frame == 240 {');s=s[:i]+'}\n'
s+='''fn c442_late(
    mut probe: ResMut<C442Probe>,
    views: Query<(&Camera,&GlobalTransform,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let camera=views.iter().find(|(cam,_,owner)|owner.character==actor && cam.is_active).map(|(_,t,_)|serde_json::json!({"yaw":t.rotation().to_euler(EulerRot::YXZ).0,"rotation":t.rotation().to_array()}));
    let ordinal=probe.frame-1;probe.camera_rows.push(serde_json::json!({"ordinal":ordinal,"camera":camera}));
    if probe.frame==240 {
        let output=std::env::var("ALS_C442_OUTPUT").expect("input direction evidence output is required");
        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240","rows":probe.rows,"camera_after_post_update":probe.camera_rows,"screenshots":0,"performance_acceptance":"paused"})).unwrap()).unwrap();
        exit.write(AppExit::Success);
    }
}
'''
(r/'observer.rs').write_text(s)
source=Path('src/als_extras/levels/manual_playground.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {';assert source.count(needle)==1
injection='''    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0/30.0)));
    app.init_resource::<C442Probe>();
    app.add_systems(First,c442_drive);
    app.add_systems(Update,c442_sample.after(bevy_als::als::dynamics::grounded_dynamics_motor_system).before(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(Last,c442_late);
'''
(r/'playground_observed.rs').write_text(source.replace(needle,injection+needle)+s)
args=json.loads(Path('docs/audit/bf144/441c/rustc-args.json').read_text());args=[x.replace('/441c/','/442/').replace('phase441c','phase442')for x in args];(r/'rustc-args.json').write_text(json.dumps(args,indent=2)+'\n')
compile_source=Path('docs/audit/bf144/441c/compile_observer.ps1').read_text().replace('/441c','/442');(r/'compile.ps1').write_text(compile_source)
run=Path('docs/audit/bf144/441c/run_observer.ps1').read_text().replace('/441c','/442').replace('ALS_C441_OUTPUT','ALS_C442_OUTPUT');(r/'run.ps1').write_text(run)
(r/'provenance.json').write_text(json.dumps({'original_manual_playground_sha256':hashlib.sha256(source.encode()).hexdigest(),'movement_input_selection_source':'AlsCharacterExample.Input_OnMove and PlayerController.GetPlayerViewPoint; target locomotion_drive_system','observer_fields_added':'actual PlayerViewYaw, current camera yaw, effective input, direction recomputation, camera after PostUpdate','source_parameter_or_direction_data_injected':False,'original_scene_retained':True},indent=2)+'\n')
print('Prepared original Playground with read-only actual camera/input fields and Update/Last phase snapshots',flush=True)
