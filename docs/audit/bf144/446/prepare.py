import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
s=Path('docs/audit/bf144/445/observer.rs').read_text().replace('C445','C446').replace('c445_','c446_')
needle='    In(before): In<bool>,'
assert s.count(needle)==1
parameters='''
    transforms:bevy::transform::helper::TransformHelper,
    history:Query<(&Transform,Has<ChildOf>,&bevy_als::als::state::LocomotionState,&bevy_als::als::state::Gait,&bevy_als::als::state::Stance,&bevy_als::als::state::RotationMode,
        &bevy_als::als::motion::AlsViewYaw,&bevy_als::als::gait::AlsDesiredGait,&bevy_als::als::transition::AlsDesiredStance,
        (&bevy_als::als::input::AlsInputCommand,Option<&bevy_als::als::motion::AlsInputOverride>,Option<&bevy_als::als::view::AlsPlayerViewYaw>,Has<bevy_als::als::motion::AlsMovementInputBlocked>,&bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>))>,
    sources:Query<&bevy_als::extensions::SourceAnimationInputs>,
    ready:Res<ManualPlaygroundStartupReady>,'''
s=s.replace(needle,needle+parameters)
s=s.replace('avatars: Query<(&ChildOf,&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance),With<bevy_als::character_assembly::AlsCharacterAvatar>>,','avatars: Query<(Entity,&ChildOf,&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance),With<bevy_als::character_assembly::AlsCharacterAvatar>>,')
s=s.replace('let Some((parent,instance))=avatars.iter().next()','let Some((avatar,parent,instance))=avatars.iter().next()')
needle='    let row=serde_json::json!({\n        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),'
assert s.count(needle)==1
extra='''    let actor=parent.parent();
    let (local,parented,locomotion,gait,stance,rotation,view,desired_gait,desired_stance,(command,input_override,player_view,blocked,controller))=history.get(actor).expect("Initial state producer components are required");
    let actor_world=transforms.compute_global_transform(actor).unwrap().to_scale_rotation_translation();
    let avatar_world=transforms.compute_global_transform(avatar).unwrap().to_scale_rotation_translation();
    let source=sources.get(instance.player_entity()).unwrap();
    let tags=["locomotion_mode","gait","stance","rotation_mode","locomotion_action"].into_iter().map(|key|(key.to_owned(),serde_json::json!(source.gameplay_tag_input(key).map(|v|v.as_str().to_owned())))).collect::<serde_json::Map<String,serde_json::Value>>();
    let effective=bevy_als::als::motion::movement_input_for_action(*input_override.map_or(command,|v|&v.0),blocked);
    let history_input=serde_json::json!({"startup_ready":ready.passed,"root_tags":tags,"locomotion":locomotion,"gait":gait,"stance":stance,"rotation_mode":rotation,
        "desired_gait":desired_gait.0,"desired_stance":desired_stance.0,"raw_view_yaw":view.0,"player_view_yaw":player_view.map(|v|v.0),
        "input_blocked":blocked,"input_override_present":input_override.is_some(),"command_move":command.move_dir.to_array(),"effective_move":effective.move_dir.to_array(),"desired_world_motion":controller.basis.desired_motion.to_array(),
        "actor_parented":parented,"local_translation":local.translation.to_array(),"local_rotation":local.rotation.to_array(),"local_scale":local.scale.to_array(),
        "actor_world_translation":actor_world.2.to_array(),"actor_world_rotation":actor_world.1.to_array(),"actor_world_scale":actor_world.0.to_array(),
        "avatar_world_translation":avatar_world.2.to_array(),"avatar_world_rotation":avatar_world.1.to_array(),"avatar_world_scale":avatar_world.0.to_array()});
'''
s=s.replace(needle,extra+needle+'\n        "history_input":history_input,')
(r/'observer.rs').write_text(s)
old=Path('docs/audit/bf144/445/playground_observed.rs').read_text()
needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle)
a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text()
injection=old[a:b].replace('C445','C446').replace('c445_','c446_')
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+s)
args=json.loads(Path('docs/audit/bf144/445/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/445/','/446/').replace('phase445','phase446')for v in args],indent=2)+'\n')
for name in ['compile.ps1','run.ps1','functional.ps1','run_python.ps1']:
    (r/name).write_text(Path('docs/audit/bf144/445',name).read_text().replace('/445','/446').replace('ALS_C445_OUTPUT','ALS_C446_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/445/observer.rs',added='Current producer states, source tags, effective input and local/world TQS from first animation query onward',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared actual cold-start input and state history observation',flush=True)
