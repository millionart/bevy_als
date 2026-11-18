# Runtime assets

The runtime reads `ALS/`, `ALSRuntime/`, and `ALSCamera/` from the Bevy asset root. The optional Playground also uses `ALSExtras/`. Each `.anim.ron` names its animation in a meshless GLB and a separate skeleton asset. The runtime samples the GLB's channels and `ALS_float_curves` extension. Additive nodes read the reference animation named in the graph. For an ALS skeleton, virtual bones follow its sampled source and target bones. Animated scenes obtain their geometry from the model assets.

Use `tools/port.py animation --gltf <Unreal export> --output <path>.glb` in the UE to Bevy Workflow repository to pack new exported animations.

Bevy derives animation player bindings from embedded glTF animation channels. For a static skinned glTF scene, ALS uses Bevy's skin joints to locate their shared model root and bind the named nodes to a Bevy animation player. The animation graph loader uses the same binding in either case.

This repository contains converted models, animations, textures, and descriptions from ALS-Refactored and Unreal Engine. The original asset licenses apply to those assets. The ALS-Refactored MIT notice is included in the repository [license](../LICENSE).
