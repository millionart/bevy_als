---
name: animation-graph-equivalence
description: Use when creating or modifying this repository's .animgraph.ron or .fsm.ron assets, or when reviewing their ALS-Refactored equivalence, graph layout, pin boundaries, or validation evidence.
---

# Animation Graph Equivalence

Use this skill for every animation graph asset change in this repository. Its purpose is to retain ALS-Refactored behavior while making the actual Bevy graph readable and auditable.

## Establish source facts first

Before changing a graph, re-extract the relevant local ALS-Refactored asset facts. Preserve the complete source graph, pins, links, properties, state transitions, clip values, and node positions in the applicable contract or extraction output. Do not use an old summary, a preview SVG, or a public web copy as the implementation source when local data is available.

docs/als-ref-animation-asset-map.md is read-only. Do not alter it.

## Source visual evidence

For every UE authored graph, collect two independent source facts before changing the target:

- First use a command-line or Unreal Python asset-open path to locate the asset. Do not use MCP or Computer Use merely to search for or open an asset when a command-line path can do it.
- Re-extract the local source asset and use extract_ue_visible_graph_contract.py to preserve the complete visible source graph, including node properties, pins, links, comments, and positions.
- Once the asset is already open, use MCP Slate Inspector screenshot capture to save the real Editor graph view under the current target evidence directory.
- If MCP screenshot capture is unavailable, record the tool failure and render the native source contract with render_ue_visible_graph.py. Computer Use is the final fallback and must follow its own skill instructions.

A target-derived manifest is only a target integrity record. It can never serve as source layout evidence or close a visual-equivalence Gate.

Use command-line extraction or a non-terminating Unreal Python entrypoint to locate an asset before any MCP use. Do not launch a temporary Editor process whose script requests Editor exit merely to open a graph: that path can tear down `AnimationBlueprintEditor` while it is active. Capture with MCP only from the already-running persistent Editor session. If its Slate tree cannot expose a child graph, retain the failure evidence and use the freshly extracted contract SVG rather than repeatedly restarting the Editor.

## Toolchain

Use the repository tools as one connected evidence chain:

- `run_ue_source_reader.ps1` produces the fresh native extraction.
- `extract_ue_authored_graph_inventory.py` discovers the complete authored closure; run it with `--write` after extraction and `--check` before the Gate.
- `extract_ue_visible_graph_contract.py` records one graph contract. For a recursive closure, first run `extract_ue_visible_graph_contracts.py --write`, then its `--check`, to materialize and verify every discovered graph, FSM, and transition contract from one fresh native payload; `render_ue_visible_graph.py` is the native-contract visual fallback. Recursive contract file names must remain bounded and platform-portable, with the full source graph identity retained in the batch index and contract body rather than encoded in a deep UE-path file name. Its self-test must exercise a deeply nested source graph path.
- `run_ue_visible_animgraph_audit.py --source <native-extraction>` checks a graph map against the freshly generated target NodeSpec. It may use `tools/` as an isolated AssetServer root for audit-only graphs; never stage those graphs under `assets/`. `audit_ue_visible_fsm.py --source <native-extraction>` does the equivalent State Machine topology and state-layout check. Its static transition assertions must use `PropertyName=value`, never an ambiguous bare value; a source `None` marker requirement may use a concrete target-ABI exception only when the map names the target runtime path.
- `run_ue_visible_animgraph_audits.py --index <generated-index> --source <native-extraction>` runs a declared set of source-visible graph maps serially, preserving one fresh-source audit result per entry; use it for a generated recursive transition set after each individual map has been materialized.
- Generate a mixed recursive audit index from the fresh contract index and source maps when a closure contains both AnimGraphs and FSMs. Its runner must dispatch graph maps to `run_ue_visible_animgraph_audit.py` and FSM maps to `audit_ue_visible_fsm.py`; the manifest validator must prove that every recursive inventory entry is contained in that index, including audit-only transition targets under `tools/`.
- Size a manifest command timeout to the declared closure. The authoring validator must keep the timeout bounded and emit a machine-readable failed command result on expiry; it must never abort the whole Gate without a report. Do not shorten a recursive audit by omitting maps merely to fit a default timeout.
- `audit_animation_transform_conversion.py` compares selected raw UE animation tracks with the target glTF keys after the verified coordinate-basis conversion. Use it for every symptom that could originate in a migrated animation transform, and include translation, rotation, scale, key count, and time grid.
- `validate_runtime_graph_asset_boundary.py --assets-root assets --check` rejects audit-only transition graphs, audit directories, and runtime references to them under the AssetServer tree.
- `create_ue_visible_animgraph_map.py` may seed a map only from an actual fresh NodeSpec. Follow it with `annotate_ue_source_node_defaults.py --write` and `--check` for supported source nodes so every unconnected default, asset, sync group, cache name, and binding has a source/target assertion; review any unsupported node manually rather than accepting an incomplete seed. This includes `BlendListByEnum`: its blend-time defaults, enum default, source property binding, and scalar ABI exception must be recorded even when its pose pins are linked.
- `reorder_ue_visible_fsm_serial.py` may normalize only the serialized state and transition order already established by a source map. It must never invent nodes, endpoints, values, or layouts.
- `validate_animgraph_authoring.py`, `validate_animgraph_editor_assets.py`, and `verify_animgraph_node_positions.py` close the manifest, RON integrity, and graph-node layout portions of the Gate.

Every recursive inventory entry must name a Gate that consumes both its source map and `--source` native extraction. A target contract, generated manifest, or a prior audit output cannot substitute for that live-source argument.

UE can regenerate ephemeral `pin_id` values when serializing the same authored graph, and raw-ID sorting can reorder an otherwise identical top-level link list. Preserve raw IDs and extracted link order for native evidence, but use the extractor's semantic canonical hash for freshness: it excludes only volatile `pin_id` and the hash derived from it, and sorts only the top-level semantic link set. It must still compare node GUID/order, pin name/order/direction/type/default/binding, links by endpoint semantics, position, static values, and every other native field. Add a self-test that changes only raw pin IDs or link-list order and passes freshness, alongside a separate semantic-change rejection test; never work around this by accepting stale contracts or disabling fresh native comparison.

When extending source-visible audit tools, add a regression self-test for every UE graph-root variant used by the workflow, including AnimGraph roots, state results, and transition results. Compare generated NodeSpec text with newline normalization only; retain strict content comparison after normalization so host line endings cannot create false failures or conceal an actual graph change.

## Authoring boundary

Keep the source-visible Pose chain, graph split, node order, node positions, and independent control edges visible in the real .animgraph.ron.

- Use an animation context only for a proven same-pose branch whose inherited F32 curve relay is visual noise.
- Curve membership must be discovered from the source/target graph ABI. Do not hard-code a closed Pose + CurveSet + Attributes payload.
- When a source-visible scalar pin is represented by a `LayeredGraphNode` child input, declare its source default on the adapter and make root-default evaluation consume that declared value. A global key alone is not an auditable fallback. The default annotator must reject a missing target default, but must not require a dynamic key when the source pin has no Property Access binding.
- When an outer visible adapter delegates a sequence evaluator or another static authored node to a child runtime graph, source static property assertions must point to that child runtime path. The outer pin mapping still carries source default and binding; do not duplicate or invent a visible outer ABI solely to make a static assertion pass.
- When a project graph captures source curves from lower-snake-case global keys while the source graph uses PascalCase curve pins, preserve both names in the boundary context. Do not silently fall back to zero for a missing behavior-bearing source curve. A focused runtime case must record the source global value and its final ControlRig-bound value for at least one nonzero curve in the affected chain.
- For a source `Layered blend per bone`, retain each visible Blend Pose pin independently and audit the ordered per-layer branch-filter bones, blend weights, mesh-space rotation flag, and root-motion flag. The default annotator must parse and compare the source and target layer arrays rather than emitting an empty property list; skeleton, virtual-bone, or LOD differences require a concrete runtime ABI exception.
- Keep time, events, control inputs, slots, physics contracts, and other independent channels separate unless the runtime boundary proves their semantics are preserved.
- When a source-visible state graph exposes only Pose but a parent runtime transition still needs time, keep the authored graph Pose-only and place a separate hidden runtime adapter behind it. The adapter must derive its output time from the evaluated Pose timestamp or another source-proven runtime value; never restore a visible time pin merely to satisfy the target engine. If a state-label driver is internalized into an FSM node, its edge state must share that node's FSM state storage rather than introduce a competing node-state type.
- Declare every non-Pose root output in the authoring manifest with a concrete reason and source/runtime evidence. Do not remove an ABI just because Unreal does not display it.
- Treat a StateResult or other graph root's lifecycle fields as behavior-bearing authored properties. When the source root resolves a nonempty lifecycle `MemberName` (or only has a fallback `FunctionName`) in StateEntry, StateExit, InitialUpdate, BecomeRelevant, or Update metadata, the source-visible map must set `require_root_property_assertions: true` and provide one `root_property_assertions` entry per resolved function. Each assertion must point to the target graph or the exact runtime implementation that consumes the lifecycle event; a matching string without a focused behavior regression is insufficient.
- The editor's empty Inputs decoration is not an asset input when the graph has no input pins or edges. Do not create fake inputs to hide it.
- Keep audit-only RON, source contracts, source-visible maps, fixtures, and generated transition expressions under `tools/`, `docs/`, or `target/`, never `assets/graphs/`. The runtime asset tree is reserved for files that AssetServer may discover and load.

For non-graph Character Base assets such as PA_Als and CR_Als, use the same source-fact, explicit-boundary, target-space, lifecycle, and real-entry evidence discipline without creating a fake graph representation.

## Control Rig solver parity

When a source Control Rig graph uses an engine solver unit, extract both the authored node contract and the matching local engine implementation before changing the target. Audit every behavior-bearing input and output, including positional solve, primary and secondary axes, secondary-axis roll weight, pole kind and space, stretch policy, blend weight, propagation behavior, and source initialization basis. A target position-only approximation is not equivalent to a source solver that also aligns bone axes or roll.

For a reusable Control Rig function, inspect every live call site as well as the inner solver node. Per-call pins can override an inner node's visible defaults, especially mirrored left/right axes; materialize and test each call-side variant instead of applying one default to all limbs.

Record each solver pin's semantic kind and space separately. A source Location is an absolute point in its declared space, while a Direction is a vector; never reuse `location - origin` as a Location input. Add a nonzero-character-translation regression for every solver that consumes a location-like pole or target, and require the source-derived per-call variants in the static profile audit rather than merely checking that profile fields exist.

For source Forward Solve nodes that write pelvis or limb transforms, derive every target writeback from the current animation input pose, never from a prior post-rig entity Transform. When the target animation system omits a channel on a frame, capture and restore the last verified animation base before the solver; add a multi-frame regression that rejects accumulated local translation, scale, or segment-length drift.

Materialize source-derived axes, units, and fixed solver settings through the repository's offline coordinate/profile generator, then audit the generated target-space values. Runtime code must consume only those target-space values; it must not invoke a source-to-target coordinate bridge.

When source animation code handles teleportation or another state-invalidating movement event, audit both sides of the target chain: source-equivalent detection/emission and the receiver/rebase/reset behavior. Match source thresholds and time windows exactly, add a boundary regression test, and include a visible real-Playground case that records the detection and resulting pose/foot-lock state. Do not assume a receiver component alone covers an absent runtime event producer.

## Required authoring contract

Create or update a manifest under tools/animgraph_authoring/. It must declare the exact graph node sequence, positions, visible edges, root IO, source-evidence hash, non-Pose root-output exceptions, static source/layout/editor checks, and the named runtime/manual Gate.

For every UE-authored graph in the recursive inventory, create a source-visible map checked through run_ue_visible_animgraph_audit.py. The wrapper regenerates the target's current Rust NodeSpec before it invokes audit_ue_visible_animgraph.py; never use a committed or earlier spec JSON as Gate evidence. When the current native extraction is available, every command must pass it with `--source`; the wrapper then rejects a committed contract that differs from the live native graph. The map must cover every source-visible non-comment node exactly once, match title, node order, position, input-pin order, output-pin order, and root IO, account for every source comment group, and reject missing source links, target nodes, source pins, root output position differences, or source static property/value differences that control the target implementation. Every unconnected nonempty source pin default must have an exact `source_default` and `target_default_contains` assertion; every node configuration value that controls the target implementation must have an explicit source/target property assertion. Splitting, collapsing, or hiding a source-visible node is a failure unless the current checklist item explicitly records a source-backed exception and its editor-visible replacement.

## Recursive subgraph closure

Build a recursive authored-subgraph inventory before implementation. It must enumerate the top graph, every reachable State Machine, every state graph, every visible transition graph, and every other reachable authored graph. For every entry record the native source graph path, target RON or FSM path, source contract/map, and the Gate command.

Generate the source side of that inventory from the fresh native extraction, then include its canonical hash as `recursive_source_inventory` in the recipe and manifest. Use `extract_ue_authored_graph_inventory.py` with the source export and root graph suffix. `validate_animgraph_authoring.py` compares the discovered source closure against the manifest exactly; it rejects an omitted source graph, an invented source graph, a mismatched source kind, a contract for a different source graph, a target not declared by the manifest, or a Gate that does not explicitly consume the entry's source map.

Before mapping any recursive entry, materialize the whole closure with `extract_ue_visible_graph_contracts.py` from that same fresh native extraction and verify it in read-only mode. Do not manually enumerate only the graph currently being edited: the batch index and its per-entry contracts are the completeness baseline, while each entry still requires its own map and live-source audit.

Treat every inventory entry as an independent source-visible Gate. A parent graph's pass, a runtime pass, a shared hidden runtime graph, or a target-only manifest cannot cover an omitted state graph. The target state RON must retain the source-visible nodes, pin names, links, static values, and positions of that state; a generic wrapper is only permitted behind those visible nodes and does not replace them.

For State Machine entries, use `audit_ue_visible_fsm.py` to compare entry state, state order/labels/graphs/positions, transition order and endpoints. Transition-node coordinates are explicitly outside the target parity contract: retain the source coordinates in the source contract, but do not require a target coordinate, add a sidecar, or modify `bevy_animation_graph` to represent them. This exception applies only to transition-node coordinates; it does not relax state positions, transition topology, transition graph coverage, transition conditions, static values, pins, or links.

An FSM map without transition property assertions must fail by static-property coverage, not be silently accepted as a topology-only map. When every source transition is represented by a shared target runtime profile, the map may use `transition_static_contract_runtime_path` only if that file contains one exact canonical static contract for each source transition and the target dispatch actually consumes the profile's behavior-bearing values such as duration or blend mode. The FSM audit must verify each full source contract independently; a generic shared string or a source-only summary is insufficient.

For every transition, audit both the transition graph and its parent `AnimStateTransitionNode` static properties. `bAutomaticRuleBasedOnSequencePlayerInState`, automatic trigger time, priority, crossfade duration, blend mode, marker requirements, inertialization, and shared-rule settings can control behavior even when the transition graph contains only a `TransitionResult`; a root default alone never proves that a transition has no behavior.

A `TransitionResult` with no upstream visible node can still carry an unconnected pin default and a UE Property Access binding. Do not generate an empty target output for that case. Preserve the source binding and default through a root passthrough/default ABI in the audit-only graph, and require the root pin map to assert both `target_binding` and the exact target default. Add a generator self-test for this zero-node bound-root form before accepting it in a recursive Gate.

UE `Slot` nodes can receive a dynamic montage from source C++ rather than a visible AnimGraph pose pin. Extract the source queue/play/stop code and slot name before implementation. The target must sample the selected runtime graph or sequence, inject a real Pose plus its source-derived weight into the Slot boundary, and prove that the source-visible Slot result changes; setting only scalar index/weight globals is not a montage implementation. When the source sequence is additive, the target Slot must use additive composition rather than linear interpolation, with a focused test that distinguishes a nonzero sampled delta, changed Slot-node output, and changed downstream final pose. If the Slot changes but a later graph discards it, record that exact consumer boundary for its owning checklist item; do not claim end-to-end parity or alter the later module without fresh source facts.

When source dynamic-montage blend-out uses inertialization, inspect the local montage `Stop` and Slot update implementation as well as the visible downstream Inertialization node. Source behavior can stop the Slot immediately and send a duration request to that visible node; do not approximate this with a scalar fade-out. Preserve the source node's reset/forward/filter settings, carry a one-shot runtime request to its actual authored ancestor, and use source-derived pose-history residuals plus velocity decay. The focused Gate must prove all four boundaries: sampled Slot mount changes the authored chain, natural montage completion emits the request, the first stopped frame retains a nonzero inertial residual, and the pose settles to the source base by the requested duration.

If source `Montage_Play` replaces an active montage in the same slot group with a standard blend-out, model the active montage stack rather than resetting one scalar player. Every fading entry must keep its source clip time, blend weight, sampled pose, and behavior-bearing curves until its own blend-out ends; combine non-additive poses by source weights, normalize only when their sum exceeds one, and retain source pose weight otherwise. A newly spawned target sampler must receive its initial global inputs before its first graph evaluation, then receive normal per-frame updates; an expected first-frame asset delay is not a graph error, while a real graph error remains a failure. Add a pure stack-weight/pose regression and a real-entry repeated-request case.

For stationary ViewDirection rotation, extract the source branch before assuming generic view-follow rotation. ALS can hold actor yaw while no movement input is present and let `RotationYawSpeed` drive the turn; preserve the source blocked-state initialization, reset triggers, and current simulation Delta. A real-entry turn case must prove curve-driven rotation, Slot completion, and inertia without overwriting the actor transform from test code.

Comment groups use comment_policy: require by default. Use comment_policy: ignore only after a user-approved active-checklist exception; preserve the source comments in the source contract and record the reason in comment_exception. Ignoring comments never relaxes node, pin, edge, state-machine, cache, or position requirements.

Run the read-only authoring gate:

~~~powershell
python tools/validate_animgraph_authoring.py --manifest <manifest> --check
~~~

Run the mandatory editor-asset repair and read-only verification after every graph edit:

~~~powershell
python tools/validate_animgraph_editor_assets.py --root assets/graphs --fix
python tools/validate_animgraph_editor_assets.py --root assets/graphs
python tools/verify_animgraph_node_positions.py --root assets/graphs --include <graph-or-directory>
~~~

Then run the scope's source audit, focused Rust/contract Gate, and the declared run_manual.bat Playground case. Record source extraction time, static output, runtime values, and real-entry outcome in the current checklist item and commit.

## Runtime visual and performance evidence

For every graph change that can affect a visible pose, additive layer, state transition, IK, Control Rig, ragdoll, root motion, or animation asset selection, run the reusable real-Playground visual Gate before declaring equivalence:

~~~powershell
pwsh tools/run_animation_visual_validation.ps1 -Case <manual-case>
python tools/validate_animation_visual_evidence.py --report <evidence-dir>/report.json --check
~~~

The report must be generated by the runtime camera and final post-propagation skeleton, not a mock or editor preview. It must retain multiple scene PNGs, key-bone transform/scale/segment-continuity checks, graph errors, wall-frame plus graph-evaluation p50/p95/max metrics, and the exact automatic scenario/phase/phase elapsed/locomotion/gait/stance/rotation-mode state for every pose and screenshot. A clean process exit or `graph_err=none` is insufficient. Any invalid transform, collapsed or implausibly scaled skeleton, abrupt joint discontinuity, invalid rendered image, missing state label, or performance-budget breach fails the Gate and leaves the offending sample and image in the evidence directory.

For a source scenario whose Control Rig branches on animation curves, the report must also record the source-global curve value and the final curve context at the rig boundary. When the source trace proves a required nonzero value, require the target value explicitly; finite zero or a missing-context fallback is a failure, not a passing default.

When a visual or performance Gate detects an anomaly, first re-extract the matching local ALS-Refactored asset, visible graph, Control Rig, or C++ algorithm and compare it with the target before editing runtime behavior. The report identifies a symptom; it never authorizes an independent optimization, compensation, smoothing change, threshold relaxation, or algorithm rewrite. Change the target only for a source-proven difference; if the source has the same behavior or the difference cannot be proven, preserve the target behavior and record the evidence.

An automatic real-entry phase that claims to cover a source state with a stationary precondition must explicitly establish that precondition: clear commanded movement and residual velocity at the phase boundary before requesting view rotation or a turn. Waiting for a duration after a moving case is not evidence that the character became stationary. Factor the phase predicate into a focused regression so a later fixture edit cannot silently reintroduce stale motion.

When a source branch depends on a mode that is ordinarily driven by user input, drive the source-equivalent input as well as requesting the mode, then assert the resulting live mode in the scenario. Directly writing a state component is insufficient when a later input-state system can restore the mode; for example, an aiming-only Rotate In Place case must hold aim through its rotate window and release it before a View Direction turn case.

After a source-proven fix, retain only the reusable source contract, regression assertion, scenario configuration, and final Gate result required by the active checklist. Do not accumulate redundant failed screenshots, transient reports, or narrative evidence once their diagnostic role is complete; the durable outcome is a test that reproduces and rejects the regression.

Use a committed baseline only when it was captured in an independent verified run. Never create or accept a baseline in the same change being evaluated.

Before diagnosing a visual Gate frame-time failure as animation or simulation cost, extract the local source project's effective frame-pacing configuration, including VSync and frame-rate limits. Match source pacing in the target real entry before comparing wall-frame p50/p95; a target default that halves presentation cadence is a source-proven runtime mismatch, not a reason to relax the Gate threshold or attribute the delay to graph evaluation.

Physics timing must be mapped by executed semantics rather than a matching setting name or numeric value. Extract the local source physics frame path and the target scheduler before translating a maximum Delta, substep limit, or asynchronous flag. Distinguish a synchronous once-per-frame step with a clamped Delta from an accumulator-driven fixed schedule that can execute zero or many steps per render. For the local ALS-Ref Chaos path with `bSubstepping=false`, `MaxPhysicsDeltaTime` clamps one frame's physics Delta to `min(frame_delta, max_delta)`; it is not a fixed-Hz request. Do not map its `1/30` default to `Time<Fixed>::set_timestep(1/30)` or retain any other timing change unless a focused real-entry regression proves matching steps-per-render, state progression, and final visual/performance evidence.

When local source facts prove a synchronous one-step physics path, use the target's supported variable-time schedule so controller/game input runs once in its normal update phase and the physics/animation pre-post chain runs once per render in its physics phase. Apply a source maximum Delta through a project-side adapter only while the target physics systems execute, then restore the normal game/animation clock before downstream graph, pose, or camera systems. Do not globally clamp virtual time to approximate a physics-only source setting. The reusable visual evidence must require one physics step per render and independent `physics`/`physics_step` timing samples, so a scheduling regression cannot be misreported as animation cost.

Do not modify bevy_animation_graph, bevy_animation_graph_editor, or a dependency revision for graph-layout parity. Use project assets, source contracts, maps, and validation tools unless a separate active checklist item authorizes a generic runtime capability change for a behavior that cannot be implemented project-side.
