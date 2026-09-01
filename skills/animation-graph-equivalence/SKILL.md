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

Use the project command-line wrapper for Editor asset opening:

~~~powershell
pwsh tools/open_ue_asset_editor.ps1 -Asset <virtual-asset-path>
~~~

## Authoring boundary

Keep the source-visible Pose chain, graph split, node order, node positions, and independent control edges visible in the real .animgraph.ron.

- Use an animation context only for a proven same-pose branch whose inherited F32 curve relay is visual noise.
- Curve membership must be discovered from the source/target graph ABI. Do not hard-code a closed Pose + CurveSet + Attributes payload.
- Keep time, events, control inputs, slots, physics contracts, and other independent channels separate unless the runtime boundary proves their semantics are preserved.
- Declare every non-Pose root output in the authoring manifest with a concrete reason and source/runtime evidence. Do not remove an ABI just because Unreal does not display it.
- The editor's empty Inputs decoration is not an asset input when the graph has no input pins or edges. Do not create fake inputs to hide it.

For non-graph Character Base assets such as PA_Als and CR_Als, use the same source-fact, explicit-boundary, target-space, lifecycle, and real-entry evidence discipline without creating a fake graph representation.

## Required authoring contract

Create or update a manifest under tools/animgraph_authoring/. It must declare the exact graph node sequence, positions, visible edges, root IO, source-evidence hash, non-Pose root-output exceptions, static source/layout/editor checks, and the named runtime/manual Gate.

For each top-level UE graph, also create a source-visible map that is checked through run_ue_visible_animgraph_audit.py. The wrapper regenerates the target's current Rust NodeSpec before it invokes audit_ue_visible_animgraph.py; never use a committed or earlier spec JSON as Gate evidence. The map must cover every source-visible non-comment node exactly once, match title, order and position, account for every source comment group, and reject missing source links, target nodes, source pins, root output position differences, or source static property/value differences that control the target implementation. Each such property/value must use an explicit source/target assertion in the map. Splitting, collapsing, or hiding a source-visible node is a failure unless the current checklist item explicitly records a source-backed exception and its editor-visible replacement.

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

Do not modify bevy_animation_graph, bevy_animation_graph_editor, or a dependency revision unless the active checklist item explicitly proves that a project-side solution cannot meet the Gate.
