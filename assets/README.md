# Runtime assets

The runtime reads `ALS/`, `ALSRuntime/`, and `ALSCamera/` from the Bevy asset root. The optional Playground also uses `ALSExtras/`. Each `.anim.ron` names its glTF animation and skeleton. The runtime samples that glTF's channels and `ALS_float_curves` extension. Keep the glTF and its buffer files together. Additive nodes read the reference animation named in the graph. For an ALS skeleton, virtual bones follow its sampled source and target bones.

Keep each `*.additive_reference.json` next to the glTF it describes. The runtime loads the Lean and Stand reference frames from the configured Bevy asset root. The Stand clip descriptor in `ALS/` points to the glTF and reference in `ALSRuntime/`.

This repository contains converted models, animations, textures, and descriptions from ALS-Refactored and Unreal Engine. The original asset licenses apply to those assets. The ALS-Refactored MIT notice is included in the repository [license](../LICENSE).
