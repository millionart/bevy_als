# Runtime assets

The runtime reads `ALS/`, `ALSRuntime/`, and `ALSCamera/` from the Bevy asset root. The optional Playground also uses `ALSExtras/`. Each `.anim.ron` names its animation in a meshless GLB and a separate skeleton asset. The runtime samples the GLB's channels and `ALS_float_curves` extension. Additive nodes read the reference animation named in the graph. For an ALS skeleton, virtual bones follow its sampled source and target bones. Animated scenes obtain their geometry from the model assets.

Use `tools/port.py animation --gltf <Unreal export> --output <path>.glb` in UEtoBevyWorkflow to pack newly exported animations. Use `tools/port.py model` with the same arguments to pack mesh geometry and skin data. Model GLBs keep external texture URIs relative to the model. Keep the texture and shader files at their installed paths.

Bevy derives animation player bindings from embedded glTF animation channels. For a static skinned glTF scene, ALS uses Bevy's skin joints to locate their shared model root and bind the named nodes to a Bevy animation player. The animation graph loader uses the same binding in either case.

## Animation notifications and Montages

An AnimSequence keeps its notify intervals, filter settings, notify parameters, and authored sync markers in the adjacent `*.notify.json`. The matching `*.anim.ron` selects animation keys and curves from the GLB. A UE AnimMontage has its own `AM_*.montage.json` in the action folder. Its segments name the graph clip and Slot and carry the start, trim, rate and loop count. Its `playback` object holds the Montage rate, blends, and action curves. Roll, Mantle, and Get Up use the existing `PostLocomotion` action playback and source root motion policies.

Edit the local JSON and run the command below with the UEtoBevyWorkflow tools. It refreshes the path-only `als_notify_manifest.json` and the generated `event_tracks` blocks in the graph clips. The index lets Bevy load the whole catalog through its configured asset source, including in builds without filesystem directory access. The event tracks give the animation graph its sampled event queue. Edit notify intervals in the local JSON to keep these two consumers in sync. Graph sync groups and blend-space phase references retain their authored graph configuration. Built-in action playback reads the Montage defaults at build time. Rebuild after editing those playback parameters.

```sh
python3 <workflow>/tools/notify_assets.py --assets-root assets --index ALS/Animations/als_notify_manifest.json
```

To re-export from UE, use `tools/port.py notify` with a UE asset path or capture and the source-to-clip map used for graph conversion. Write a sequence to `*.notify.json` or a Montage to `*.montage.json`. Run the index and track command after installing the record. Its `--check` mode checks that the installed graph tracks match.

## Asset ownership

| Files | Consumer |
| --- | --- |
| Animation and model GLBs, `*.anim.ron`, `*.animscn.ron`, `*.skn.ron`, graph, physics, curve and settings assets | ALS, Bevy and the animation graph. The scene and skeleton paths stay stable. |
| Textures, shader sources, and `*.rgba16f` | Rendering and Playground assets. |
| `installation.manifest.json` | Generated graph file ownership and asset tests. Keep these next to the installed graphs for re-exports. |
| `ALS/OverlayObjects/Bow/bow.json` | Input for the overlay actor converter. The installed runtime reads `overlay_actor.runtime.json`. |
| `Validation/` | Standalone validation scenes and graphs with existing asset paths. |
| [Control Rig program fixture](../src/tests/fixtures/rig/cr_als_program.json), [moving-object curve fixture](../src/tests/fixtures/playground/moving_object_curve_samples.json) | Rust regression tests. The installed graph carries the runtime rig program, and the Playground uses the rich movement curve. |

Generated HUD and menu Rust fragments live beside their Rust consumers in `src/`. When replaying the historical HUD importer, pass `--output` with the path used by the corresponding `include!` call.

This repository contains converted models, animations, textures, and descriptions from ALS-Refactored and Unreal Engine. The original asset licenses apply to those assets. The ALS-Refactored MIT notice is included in the repository [license](../LICENSE).
