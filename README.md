# Advanced Locomotion System for Bevy

`bevy_als` is a third-person character plugin for Bevy 0.20.1, based on [ALS-Refactored](https://github.com/Sixze/ALS-Refactored). It provides grounded and airborne movement, crouching, rolling, mantling, layered animation, foot IK, ragdoll recovery, footprints, and a third-person camera.

## Integrate the plugin

1. Add `bevy_als` to your Cargo dependencies and use the matching Bevy version. Install Bevy's `DefaultPlugins` before `bevy_als::AlsPlugin`.
2. Spawn an actor with `spawn_als_character` and `AlsCharacterBlueprint`. Set `spawn_camera: true` to create an ALS view camera. For a host camera that should render footprints, attach Bevy's `DepthPrepass`.
3. Write `AlsInputCommand` on each controlled actor in `PreUpdate`. Refresh movement and held values every frame. Set action pulses such as `jump_pressed` for one update. Use `AlsStateRequest` for direct state requests.

The default character, camera, animation, and footprint assets are embedded by the plugin. Games can load a bundled asset with `als_asset_path` or use their own asset paths for custom content.

ALS advances physics in Avian's default `FixedPostUpdate` schedule. ALS buffers press pulses until a physics step consumes them. When the host runs physics in `PreUpdate`, order its input writers before `AlsFrameInputSet` in that schedule. The character graph, cosmetic layers, IK and camera update once per display frame in `PostUpdate`. The roll slot samples physical displacement on each physics step.

Ordinary `AnimationGraphPlayer` entities sample on the configured physics schedule and write their display poses in `PostUpdate`. If the host installs `AnimationGraphPlugin` first, use matching schedules and the graph plugin's standard playback. Hosts can mark dedicated physics input graphs with `extensions::post_physics_animation::PhysicsAnimationPlayer` to sample inside each actual solver step.

ALS enables Avian transform interpolation on characters when it installs the default fixed-step physics world. The solver owns `Position` and `Rotation`, and cameras and meshes consume the interpolated `Transform`. A host that installs Avian selects its own interpolation policy.

To select a different physics schedule, insert `AlsPhysicsSchedule::new(schedule)` before adding `AlsPlugin`. If the host installs Avian itself, use `AlsPlugin::physics_plugins()` for the default or `AlsPlugin::physics_plugins_in(schedule)` for a custom schedule. Both install ALS's ragdoll collision hooks.

Insert `AlsPhysicsSchedule::SourceFrameTiming` before adding `AlsPlugin` to opt in to the original Unreal-style frame timing and world timestep cap in `PostUpdate`. An ALS-owned world also uses the source contact and substep settings. If the host installs Avian, its solver configuration and `SubstepCount` apply. Mark the local player with `character_assembly::AlsPlayerCharacter` when using ALS player diagnostics.

Hosts with their own Avian contact rules can install `AlsPlugin::with_collision_hooks::<HostHooks>()`. Compose the host's filter with `bevy_als::extensions::RagdollCollisionHooks` to retain ALS ragdoll and pawn exclusions. If Avian is already installed, use the same hook type and schedule in both plugins.

Run `cargo run --example als_character` from the companion `bevy_als_playground` project to exercise ALS with WASD, mouse look, and jump. The core crate is a library. The full Playground scene owns its Enhanced Input adapter.

## Assets and conversion

An animation `.anim.ron` file references its meshless GLB and skeleton. Character models use separate GLBs. Animation notifications and sync markers sit beside their sequences in `*.notify.json`. Montage settings and notifications live in `*.montage.json`. Regenerate the path index and event tracks after editing notifications. See [UEtoBevyWorkflow](https://github.com/millionart/UEtoBevyWorkflow) for conversion tools.

Sequence sampling honors logical source frames for GLB `STEP` keys, including uniform timestamps derived from either frame rate or clip duration. Preserve the rich curve evaluation order, native CurveFloat precision, and get-up montage curve data when re-exporting. The installed high mantle montage retains the original curve data. The measured key differences from re-export were at most about 4.77 × 10⁻⁷ seconds and 3.95 × 10⁻⁹ in curve value.

Use the animation graph library's `ConstF32` for new graphs. Bundled graphs retain `als_ext::ConstantF32Node` and its `value` output. Review time propagation and duration selection when migrating a graph that uses `als_ext::BlendSpace1DNode` to the built-in blend space.

## Project layout

| Directory | Responsibility |
| --- | --- |
| `src/als/` | Character motion, state, animation, actions, IK, and notifications |
| `src/als_camera/` | ALS camera, settings, and camera shakes |
| `src/animation_graph_ext/` | ALS graph nodes, sequence clocks, sync groups, slots, curves, and rig execution |
| `src/control_rig_ext/` | ALS rig units |
| `src/avian_ext/` and `src/tnua_ext/` | Physics queries, ragdoll handoff, and controller support |
| `src/tests/` | Core behavior, asset loading, and regression fixtures |

The runtime uses Bevy's ForwardDecal for footprints. Camera and animation timing use Bevy's real and virtual clocks. Network prediction and replication, Unreal editor tools, MetaSounds, Niagara, and general motion matching are outside the plugin's scope.

## Animation quality settings

The nearest active 3D camera selects the default bone LOD. At 32 meters the animation skips finger branches. At 72 meters it also bypasses Control Rig evaluation. Set `AlsAnimationLodSettings` to change the bands, protect socket bones, or filter scalar curves by LOD and linked bones. Imported Control Rig, additive and layered blend nodes accept an optional `lod_threshold`.

Set `AlsAnimationRateSettings.enabled` to stagger updates for visible distant characters and interpolate the displayed pose. The default distance bands update every third frame at 32 meters and every sixth frame at 72 meters. Set `cpu_budget_ms` to adapt evaluation rates and interpolation to an average CPU budget. `AlsAnimationSignificance` overrides distance priority, and `reduce_rig_under_budget` permits rig bypass during budget pressure. Locally controlled characters and actions that require a continuous pose take priority.

`AlsAnimationSharingSettings` enables sharing identical clip samples by default. The cache matches the clip, sample time and required bones. For visual-only characters with matching bone paths, attach `AlsAnimationPoseFollower` to the entity referenced by their `AnimatedBy` components. Their leader supplies the pose and owns playback, while each follower keeps its own world transform.

Set `AlsPreRigSharingSettings.enabled` at startup to share compatible stock ALS graph updates before Control Rig. Each character retains its own Rig and physics state. The scheduler matches the animation inputs, pose detail and state history as playback starts. A character with independent input or animation state uses ordinary graph evaluation.

The default `AlsAclAnimations` decodes bundled ACL locomotion clips and the baked local-space idle additive. The idle decoder uses the base pose and frame declared by the sequence player. Insert `AlsAclAnimations::empty()` to use the installed glTF animation tracks for every clip.

To rebuild the bundled ACL bank, request a `SequenceCompressionExport` from the native UE reader with `relative_retarget` enabled and the bundled sequence paths. Pack the resulting capture from the project root. Pass a `SequencePoseProbe` capture through `--additive-base-probe` for each local additive clip to bind its authored frame-zero base.

```sh
python3 tools/import/pack_als_acl.py capture.json assets/ALSRuntime/Animations/als_acl.bank
```

Run the core tests with `cargo test --locked --jobs 2 -- --test-threads=1`.

## License

The code is MIT licensed. Bundled ALS-Refactored and Unreal Engine assets retain their original licenses. See [LICENSE](LICENSE).
