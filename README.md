# Advanced Locomotion System for Bevy

`bevy_als` is a third-person character plugin for Bevy 0.20.1, based on [ALS-Refactored](https://github.com/Sixze/ALS-Refactored). It provides grounded and airborne movement, crouching, rolling, mantling, layered animation, foot IK, ragdoll recovery, footprints, and a third-person camera.

## Integrate the plugin

1. Add `bevy_als` to your Cargo dependencies and use the matching Bevy version. Install Bevy's `DefaultPlugins` before `bevy_als::AlsPlugin`.
2. Copy `assets/ALS`, `assets/ALSCamera`, and `assets/ALSRuntime` from the same ALS revision into your application's Bevy asset root. Preserve the directory names and relative paths.
3. Spawn an actor with `spawn_als_character` and `AlsCharacterBlueprint`. Set `spawn_camera: true` to create an ALS view camera. For a host camera that should render footprints, attach Bevy's `DepthPrepass`.
4. Write `AlsInputCommand` on each controlled actor in `PreUpdate`. Refresh movement and held values every frame. Set action pulses such as `jump_pressed` for one update. Use `AlsStateRequest` for direct state requests.

If the host installs Avian itself, register `AlsPlugin::physics_plugins()` before `AlsPlugin` to use the matching physics schedule and collision hooks. Mark the local player with `character_assembly::AlsPlayerCharacter` when using ALS player diagnostics.

The companion `bevy_als_playground` project contains the integration guide and a minimal application with WASD, mouse look, and jump. Its full scene uses an Enhanced Input adapter owned by the Playground.

## Assets and conversion

An animation `.anim.ron` file references its meshless GLB and skeleton. Character models use separate GLBs. Animation notifications and sync markers sit beside their sequences in `*.notify.json`. Montage settings and notifications live in `*.montage.json`. Regenerate the path index and event tracks after editing notifications. See [UEtoBevyWorkflow](https://github.com/millionart/UEtoBevyWorkflow) for conversion tools.

Sequence sampling honors logical source frames for GLB `STEP` keys, including uniform timestamps derived from either frame rate or clip duration. Preserve the rich curve evaluation order, native CurveFloat precision, and get-up montage curve data when re-exporting. The installed high mantle montage retains the original curve data. The measured key differences from re-export were at most about 4.77 × 10⁻⁷ seconds and 3.95 × 10⁻⁹ in curve value.

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

Run the core tests with `cargo test --locked --jobs 2 -- --test-threads=1`.

## License

The code is MIT licensed. Bundled ALS-Refactored and Unreal Engine assets retain their original licenses. See [LICENSE](LICENSE).
