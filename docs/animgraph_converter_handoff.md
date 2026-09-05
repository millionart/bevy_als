# AnimBP converter handoff

This report covers the fresh direct conversion corpus generated on 2026-09-05 for `AB_Als_Standing`, `AB_Als_Crouching`, `AB_Als_Grounded`, `AB_Als_Locomotion`, `AB_Als_Layering`, `AB_Als_Head`, `AB_Als_Ragdolling`, and `AB_Als_Default`.

All eight source records converted with zero blockers. Their 150 reachable graphs and 1051 reachable nodes passed deterministic regeneration, editor structure, node-position, and runtime asset-boundary checks. The generated closure's own `integration.report.json` and `integration.report.md` are the authoritative per-asset wiring contracts; keep them beside the generated RON closure when it is integrated.

All 248 generated `.animgraph.ron` files also loaded through the real Bevy asset loader with `AlsExtPlugin`; their 949 NodeLike specs and 1716 edges resolved without a loader or spec error. This is deliberately not a visual Playground claim: the generated graphs fail closed until the source-derived providers below are connected.

## Manual graph comparison

Standing, Crouching, and Grounded were compared against their existing manual topology records, with fresh UE source as the arbiter. The converter passed all three comparisons with no source-class blocker.

- Standing source has six `K2Node_AnimGetter` nodes and two `K2Node_Knot` nodes; the manual topology has two of each. The converter covers all source nodes.
- Crouching source has four `K2Node_AnimGetter` nodes and two `K2Node_Knot` nodes; the manual topology has none. The converter covers all source nodes.
- Grounded has no manual capability gap. Its two source stance-change transitions resolve the three-key custom curve contract.

Manual file/node counts are therefore historical evidence only. They must not create aliases, flattened names, source-path special cases, or fallback wiring in the converter.

## Actor and runtime functions

No external function is unresolved. The following source calls are outside an AnimBP graph, but already map to target providers. Ensure the listed provider plugin/system is installed and active; do not recreate the function inside generated RON.

| Source function | Assets | Existing target provider |
| --- | --- | --- |
| `InitializeGrounded` / `ResetGroundedEntryMode` | Grounded | `GroundedLayerPlugin` |
| `RefreshGrounded` | Grounded | `AlsActualMotionPlugin`, `StandingLayerPlugin`, `CrouchingLayerPlugin` |
| `InitializeHead` / `RefreshHead` | Head | `HeadLayerPlugin` |
| `InitializeLean` / `RefreshInAir` | Locomotion | `als_locomotion_initialize_lean`, `als_locomotion_refresh_in_air` |
| `InitializeStandingMovement` / `RefreshStandingMovement` / `ResetPivot` | Standing | `StandingLayerPlugin` |
| `RefreshCrouchingMovement` | Crouching | `CrouchingLayerPlugin` |
| `RefreshGroundedMovement` / `SetHipsDirection` / `RefreshDynamicTransitions` | Standing, Crouching | `StandingLayerPlugin`, `CrouchingLayerPlugin` |
| `InitializeTurnInPlace` / `RefreshTurnInPlace` | Standing, Crouching | `TurnLayerPlugin` |
| `RefreshRotateInPlace` | Standing, Crouching, Locomotion | `als_turn_layer_refresh_rotate_in_place` |

The 13 source AnimBP function-graph call sites are materialized or mapped already: `StopTransitionAndTurnInPlaceAnimations`, `PlayStopLeftTransitionAnimation`, `PlayStopRightTransitionAnimation`, `PlayStopTransitionAnimation`, and `PlayRollToGroundedTransitionAnimation`.

## Required integration wiring

These are runtime data providers, not unknown converter features. Use the exact source-derived keys in each generated `integration.report.json`; do not add aliases.

- Write the 208 declared property inputs to the `AnimationGraphPlayer` global-input store before graph evaluation.
- Supply recorded state/machine weights for `GetInstanceStateWeight` and `GetInstanceMachineWeight` through `AnimationGraphPlayer::set_global_input_data`.
- Advance source sync-group phase and `animation.delta_seconds` for the ten declared groups: Standing `First Pivot`, `Movement`, `Run Start`, `Second Pivot`; Crouching `Movement`; Locomotion `Fall`, `Flail`, `Jump`, `Land`; Default `Secondary Motion`.
- Set gameplay tags with `als_ext::set_gameplay_tag_input` for the five source-derived keys: `gait`, `grounded_entry_mode`, `locomotion_action`, `locomotion_mode`, and `stance`.
- Provide nine named slot channels: Standing `TurnInPlaceStanding`, Crouching `TurnInPlaceCrouching`, and Layering `Head`, `Spine`, `Pelvis`, `Curves`, `ArmRight`, `Legs`, `ArmLeft`.

There are zero unhandled property bindings, external graph dependencies, and other conversion blockers in this corpus.
