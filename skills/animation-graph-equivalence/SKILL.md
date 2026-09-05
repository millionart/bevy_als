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
4. A hidden Pose-only proxy inherits context from its child Pose by data type, not output-pin spelling. Supply a missing curve only when source proves it is a downstream default; do not demand that downstream-generated curve from an upstream proxy.
5. Use a manifest in `tools/animgraph_authoring/` for every changed scope. It must cover the recursive source closure and point each entry at fresh source evidence, source-visible maps, static audits, editor checks, focused tests, and the declared manual case.
6. Run the scope commands before runtime: `tools/validate_animgraph_authoring.py --check`, `tools/validate_animgraph_editor_assets.py` repair plus read-only validation, `tools/verify_animgraph_node_positions.py`, `tools/import/run_ue_visible_animgraph_audit.py`, `tools/import/audit_ue_visible_fsm.py`, `tools/validate_runtime_graph_asset_boundary.py`, and affected tool self-tests. Comment groups default to required; ignore them only when the active checklist item records the user's exception.
7. Keep a current closure ledger under the active checklist item: total/covered source subgraphs, last passed static audit, and the one open source-chain boundary. Update it before runtime or scope expansion. Include an auxiliary consumer only when a source-proven edge or function requires it and name that dependency in the ledger; otherwise defer it.

## Generator maintenance

1. Treat source-contract generation as the default authoring path. Its public source input is a `.uasset`; the converter itself invokes the repository native UE source reader and uses any JSON only as a temporary internal intermediate. The generator may emit source-visible graphs, hidden runtime children, FSM aliases/conduits, maps, manifests, and validation fixtures; generated runtime assets remain committed project assets and never become a runtime source-to-target bridge. Registry dispatch may use only source node class, pin schema, function/lifecycle signature, asset category, or another source-proven reusable semantic; it must not branch on a particular AnimBP name, source asset path, or one graph's node GUID.
2. An unregistered source node, function, pin type, property, transition form, or lifecycle call is an agent-owned generator-maintenance task, not a request for the user to hand-map or hand-edit a target graph. It must emit a machine-readable error and block IR/RON/FSM/materialized output. Re-extract the local source fact, identify the smallest reusable mapping or project extension, add its source contract fixture and self-test, then regenerate the target and rerun the source audit.
3. Do not add a one-off target-only substitute merely to finish an asset. If an ALS-specific semantic implementation is required, register it explicitly in the generator mapping and preserve its source/runtime boundary in the generated manifest.
4. Only an unavailable source asset, missing authority, or a proven target capability boundary may block automation. Record the exact unsupported source identity and evidence; do not silently fall back to a partial graph or ask the user to complete the mapping.
5. Once the project declares a source-diverse golden corpus passed by one-click generation and all declared static/runtime Gates, every later source-equivalence graph task is converter-first. Re-extract the fresh source, update the registry when unsupported, generate and verify the target, and only materialize generated assets; do not resume graph-by-graph hand authoring. This never removes source fact, lifecycle, asset, coordinate, or real-entry validation.
6. A graph tree is convertible only when its complete reachable target dependency closure can load. A partial root/FSM tree with missing state or transition artifacts must not be emitted as RON or used for editor/spec/runtime proof; retain only its IR and grouped unsupported report until the full closure is materialized.
7. A native source reader invoked by the converter must have a bounded wall/no-progress watchdog, emit a machine-readable result with its last progress, and keep a focused self-test for both timeout and normal completion. An absent or stale reader output is a failed source fact, never a successful conversion.

## Runtime behavior

1. Complete source-to-target static closure before one coherent real Playground run. Use focused fixtures and tool self-tests for detector design; execute `tools/run_animation_visual_validation.ps1` and `tools/validate_animation_visual_evidence.py` only after that closure is ready. Run its no-driver `startup` case before motion cases; it must reject missing or zero-channel root Pose even without a graph error. Do not repeatedly launch the game to guess a root cause.
2. A dynamic window declares source state/clip, stable precondition, expected actual FSM state, expected direction/variant, and whether pre-rig/final pose motion, root-pose activity, actor yaw, or lifecycle is required.
3. When a source action branches on live stance or another mutable state, derive the fixture branch from the same source setting and lifecycle, not its requested initial state. If actual graph-state telemetry is absent, repair that reader before accepting a requested/runtime mirror as evidence.
4. Bind graph telemetry to the exact authored target node identity or title, including whitespace and line breaks; never use a guessed short label. Add a focused reader regression when that identity differs from a human shorthand.
5. Evidence records actual rather than requested graph state, root-pose channel activity, pre-rig and final local-pose motion, target variant, curve/clip/weight trace, scenario labels, camera frames, skeleton continuity, and timing. Distinct source branches need source-derived inputs and distinct canonical pose signatures.
6. The common temporal guard runs independently of project algorithms at pre-rig and final stages. It rejects non-finite values, scale/segment failures, local spikes, position spikes, and limb-plane flips. Rebase only at an explicit source fixture boundary.
7. For signed turn/rotate behavior, prove target-space yaw sign as well as magnitude, asset variant, and lifecycle. A correct-looking clip with reversed physical yaw fails.
8. When the visual collector is active, a manual scenario verdict must wait for its required frames and source-window captures to be written. Fast scenario completion may not produce a partial report; keep a focused readiness regression.
9. A completed manual scenario whose visual capture count no longer advances must be ended by a fixed-progress watchdog. Its JSON report records the count, required count, wait and reason; test both continued capture after settlement and watchdog termination.

## Failure reports and repair

1. The first graph evaluation error must write `report.json` before exit, including `primary_failure`, raw error, pin/node identity, active graph/context candidates, ambiguity status, root graph/dependency state, root-pose classification, scenario labels, and screenshot attempt status. Every driver run also records its exit code and complete ManualIntegration verdict with individual coverage labels and final diagnostic details; the runner must audit that report even when the game exits nonzero.
2. A missing root Pose or zero active channels is classified as reference/A-pose risk whether or not a graph error occurred; it is a reportable root cause, not merely a generic failed run.
3. Repair the first source-chain boundary that violates its source dynamic invariant. Downstream IK/Control Rig ablation only proves sensitivity, never root cause.
4. After a source-proven repair, add the smallest regression or self-test that would reject the old false pass, then keep only the reusable contract and final evidence.

## Boundaries

Use project assets, `crates/als-ext`, and project-side adapters before considering dependency changes. Preserve source lifecycle and coordinate semantics, but do not import UE inheritance or reflection architecture.
