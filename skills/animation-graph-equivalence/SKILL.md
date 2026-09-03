---
name: animation-graph-equivalence
description: Use when creating, changing, or auditing this repository's animation graphs, state machines, graph layout, or ALS-Refactored animation behavior.
---

# Animation Graph Equivalence

Keep the authored Bevy graph readable and source-equivalent without changing dependencies or disguising behavior in runtime adapters.

## Scope

Use this skill for `.animgraph.ron`, `.fsm.ron`, source-visible graph review, or a runtime change that can alter graph-selected pose, curve, state, slot, IK, Control Rig, root motion, or animation asset selection.

Read the current checklist item first. Do not alter `docs/als-ref-animation-asset-map.md`, a completed/archived item, or `bevy_animation_graph` unless the active item explicitly authorizes it.

## Source evidence

1. Re-extract the local source asset and its relevant C++/settings facts before implementation. Native extraction is the full contract; live Editor MCP is an independent read-only fact.
2. Reuse a healthy persistent Editor. Open the asset with `EditorToolset.EditorAppToolset.OpenEditorForAsset`, then read it through asset load, graph list, graph detail, and DSL APIs. A healthy empty DSL or partial canvas is a capability limitation, not evidence of an empty graph.
3. For visible graph parity, record node order, title, pins, links, static/default values, comments, root IO, and positions. Build the recursive reachable closure: top graph, FSMs, states, transition graphs, and linked authored subgraphs.
4. Only a complete canvas capture or a provably complete stitch proves layout. Otherwise retain the MCP failure fact and use the same-run native contract; whole-window screenshots do not prove graph layout.
5. Experimental source-node warnings require a source-visible note and future recheck; they are not a parity exception.

## Static parity

1. Keep source-visible nodes, Pose/control edges, positions, root IO, cache scope, state order, transition endpoints, and behavior-bearing defaults visible in the target. A hidden runtime adapter may support a source-visible node but cannot replace it.
2. Do not fabricate Inputs, erase independent time/event/control/physics channels, or use a closed hard-coded pose payload. Discover the actual ABI. A non-Pose root output needs a concrete source/runtime reason in the manifest.
3. raw source exports remain immutable. For a coordinate- or unit-dependent algorithmic scalar, preserve the raw source curve and generate separate offline target-space data; runtime consumers read only that target data. Never append derived fields to a raw source export or hide conversion in a runtime bridge.
4. Use a manifest in `tools/animgraph_authoring/` for every changed scope. It must cover the recursive source closure and point each entry at fresh source evidence, source-visible maps, static audits, editor checks, focused tests, and the declared manual case.
5. Run the scope commands before runtime: `tools/validate_animgraph_authoring.py --check`, `tools/validate_animgraph_editor_assets.py` repair plus read-only validation, `tools/verify_animgraph_node_positions.py`, `tools/import/run_ue_visible_animgraph_audit.py`, `tools/import/audit_ue_visible_fsm.py`, `tools/validate_runtime_graph_asset_boundary.py`, and affected tool self-tests. Comment groups default to required; ignore them only when the active checklist item records the user's exception.

## Runtime behavior

1. Complete source-to-target static closure before one coherent real Playground run. Use focused fixtures and tool self-tests for detector design; execute `tools/run_animation_visual_validation.ps1` and `tools/validate_animation_visual_evidence.py` only after that closure is ready. Do not repeatedly launch the game to guess a root cause.
2. A dynamic window declares source state/clip, stable precondition, expected actual FSM state, expected direction/variant, and whether pre-rig/final pose motion, root-pose activity, actor yaw, or lifecycle is required.
3. Evidence records actual rather than requested graph state, root-pose channel activity, pre-rig and final local-pose motion, target variant, curve/clip/weight trace, scenario labels, camera frames, skeleton continuity, and timing. Distinct source branches need source-derived inputs and distinct canonical pose signatures.
4. The common temporal guard runs independently of project algorithms at pre-rig and final stages. It rejects non-finite values, scale/segment failures, local spikes, position spikes, and limb-plane flips. Rebase only at an explicit source fixture boundary.
5. For signed turn/rotate behavior, prove target-space yaw sign as well as magnitude, asset variant, and lifecycle. A correct-looking clip with reversed physical yaw fails.

## Failure reports and repair

1. The first graph evaluation error must write `report.json` before exit, including `primary_failure`, raw error, pin/node identity, all active graph candidates, ambiguity status, root graph/dependency state, root-pose classification, scenario labels, and screenshot attempt status. The runner must audit that report even when the game exits nonzero.
2. A missing root Pose or zero active channels after a graph error is classified as reference/A-pose risk; it is a reportable root cause, not merely a generic failed run.
3. Repair the first source-chain boundary that violates its source dynamic invariant. Downstream IK/Control Rig ablation only proves sensitivity, never root cause.
4. After a source-proven repair, add the smallest regression or self-test that would reject the old false pass, then keep only the reusable contract and final evidence.

## Boundaries

Use project assets, `crates/als-ext`, and project-side adapters before considering dependency changes. Preserve source lifecycle and coordinate semantics, but do not import UE inheritance or reflection architecture.
