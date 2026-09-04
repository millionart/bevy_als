import argparse
import json
import re


FORMAT = "ue_animgraph_root_reachability_v1"

GRAPH_KINDS = {
    "/Script/AnimGraph.AnimationGraph": "graph",
    "/Script/AnimGraph.AnimationStateGraph": "graph",
    "/Script/AnimGraph.AnimationStateMachineGraph": "state_machine",
    "/Script/AnimGraph.AnimationTransitionGraph": "transition",
}

ROOT_CLASSES = {"/Script/AnimGraph.AnimGraphNode_Root"}
RESULT_CLASSES = {
    "/Script/AnimGraph.AnimGraphNode_StateResult",
    "/Script/AnimGraph.AnimGraphNode_TransitionResult",
}
FSM_STRUCTURAL_CLASSES = {
    "/Script/AnimGraph.AnimStateEntryNode",
    "/Script/AnimGraph.AnimStateNode",
    "/Script/AnimGraph.AnimStateAliasNode",
    "/Script/AnimGraph.AnimStateConduitNode",
    "/Script/AnimGraph.AnimStateTransitionNode",
}
STATE_MACHINE_CLASSES = {"/Script/AnimGraph.AnimGraphNode_StateMachine"}
CACHE_USE_CLASSES = {"/Script/AnimGraph.AnimGraphNode_UseCachedPose"}
CACHE_SAVE_CLASSES = {"/Script/AnimGraph.AnimGraphNode_SaveCachedPose"}
EXTERNAL_NODE_KINDS = {
    "/Script/AnimGraph.AnimGraphNode_LinkedAnimGraph": "linked_anim_graph",
    "/Script/AnimGraph.AnimGraphNode_LinkedAnimLayer": "linked_anim_layer",
    "/Script/AnimGraph.AnimGraphNode_ControlRig": "control_rig",
}
FUNCTION_GRAPH_CLASSES = {"/Script/Engine.EdGraph"}
ANIM_FUNCTION_CLASSES = {"/Script/AnimGraph.AnimGraphNode_CallFunction"}
LIFECYCLE_FUNCTION_FIELDS = {
    "InitialUpdateFunction",
    "BecomeRelevantFunction",
    "UpdateFunction",
    "StateEntryFunction",
    "StateFullyBlendedInFunction",
    "StateExitFunction",
    "StateFullyBlendedOutFunction",
}


class ReachabilityError(ValueError):
    pass


def source_graph_suffix(source, graph):
    asset_path = source.get("asset_path")
    object_path = graph.get("object_path")
    prefix = f"{asset_path}:"
    if not isinstance(asset_path, str) or not asset_path:
        raise ReachabilityError("source asset path is invalid")
    if not isinstance(object_path, str) or not object_path.startswith(prefix):
        raise ReachabilityError("source graph path is invalid")
    return object_path[len(prefix) :]


def object_references(value):
    if not isinstance(value, str):
        return []
    return re.findall(r"'(/[^']+)'", value)


def node_property(node, name):
    values = [
        property_value.get("exported_text")
        for property_value in node.get("properties", [])
        if property_value.get("name") == name
        and isinstance(property_value.get("exported_text"), str)
    ]
    if len(values) != 1:
        return None
    return values[0]


def node_identity(node):
    value = node.get("object_path")
    if not isinstance(value, str) or not value:
        raise ReachabilityError("source node path is invalid")
    return value


def node_class(node):
    value = node.get("class_path")
    if not isinstance(value, str) or not value:
        raise ReachabilityError("source node class is invalid")
    return value


def animation_function_references(node):
    references = []
    class_path = node_class(node)
    if class_path in ANIM_FUNCTION_CLASSES:
        title = node.get("title")
        if isinstance(title, str):
            name = title.splitlines()[0].strip()
            if name:
                references.append(("anim_node", name))
    for property_value in node.get("properties", []):
        value = property_value.get("exported_text")
        if not isinstance(value, str):
            continue
        for field_name in LIFECYCLE_FUNCTION_FIELDS:
            match = re.search(
                rf"{re.escape(field_name)}=\(FunctionName=\"([^\"]+)\"",
                value,
            )
            if match is not None:
                references.append((field_name, match.group(1)))
    return sorted(set(references))


def incoming_links(graph):
    explicit = graph.get("links")
    if explicit is not None:
        if not isinstance(explicit, list):
            raise ReachabilityError("source graph links are invalid")
        return [
            (
                link.get("source", {}).get("node_object_path"),
                link.get("target", {}).get("node_object_path"),
            )
            for link in explicit
            if isinstance(link, dict)
        ]
    values = []
    for target in graph.get("nodes", []):
        target_path = node_identity(target)
        for pin in target.get("pins", []):
            if pin.get("direction") != "Input":
                continue
            links = pin.get("linked_to")
            if not isinstance(links, list):
                raise ReachabilityError("source input pin links are invalid")
            for source in links:
                if not isinstance(source, dict):
                    raise ReachabilityError("source input pin link is invalid")
                values.append((source.get("node_object_path"), target_path))
    return values


class RootTraversal:
    def __init__(self, source, root_suffix):
        self.source = source
        self.root_suffix = root_suffix
        self.graphs = {}
        self.all_graphs = {}
        self.function_graphs = {}
        self.nodes = {}
        self.node_graph = {}
        self.node_rank = {}
        self.incoming = {}
        self.graph_entries = []
        self.graph_entry_by_path = {}
        self.visited_nodes = set()
        self.external_dependencies = []
        self.function_dependencies = []
        self.issues = []
        self._index_source()

    def _index_source(self):
        source_graphs = []
        for graph in self.source.get("graphs", []):
            class_path = graph.get("class_path")
            if class_path not in GRAPH_KINDS and class_path not in FUNCTION_GRAPH_CLASSES:
                continue
            graph_path = graph.get("object_path")
            suffix = source_graph_suffix(self.source, graph)
            if graph_path in self.all_graphs or suffix in self.all_graphs:
                raise ReachabilityError("source graph is duplicated")
            graph["_source_graph_suffix"] = suffix
            self.all_graphs[graph_path] = graph
            self.all_graphs[suffix] = graph
            if class_path in FUNCTION_GRAPH_CLASSES:
                self.function_graphs.setdefault(graph.get("name"), []).append(graph)
                continue
            source_graphs.append(graph)
            self.graphs[graph_path] = graph
            self.graphs[suffix] = graph
            for node in graph.get("nodes", []):
                path = node_identity(node)
                if path in self.nodes:
                    raise ReachabilityError("source node is duplicated")
                self.nodes[path] = node
                self.node_graph[path] = graph_path
                self.node_rank[path] = (graph.get("index", -1), node.get("index", -1), path)
        for graph in source_graphs:
            for source_node, target_node in incoming_links(graph):
                if source_node not in self.nodes or target_node not in self.nodes:
                    self.record_issue(
                        "source_pin_link_node_missing",
                        source_graph=graph["_source_graph_suffix"],
                        source_node=source_node,
                        target_node=target_node,
                    )
                    continue
                self.incoming.setdefault(target_node, []).append(source_node)
        for values in self.incoming.values():
            values[:] = sorted(set(values), key=lambda value: self.node_rank[value])

    def graph_children_for_node(self, node_path):
        prefix = node_path + "."
        values = [
            graph
            for graph_path, graph in self.graphs.items()
            if ":" in graph_path and graph_path.startswith(prefix)
        ]
        values.sort(key=lambda graph: (graph.get("index", -1), graph["_source_graph_suffix"]))
        return values

    def record_issue(self, kind, **values):
        self.issues.append({"kind": kind, **values})

    def visit_graph(self, graph, parent_source_graph=None, reason="root"):
        graph_path = graph["object_path"]
        if graph_path in self.graph_entry_by_path:
            return
        kind = GRAPH_KINDS[graph["class_path"]]
        entry = {
            "source_graph": graph["_source_graph_suffix"],
            "kind": kind,
            "class_path": graph["class_path"],
            "name": graph.get("name"),
            "index": graph.get("index"),
            "parent_source_graph": parent_source_graph,
            "reason": reason,
            "reachable_nodes": [],
        }
        self.graph_entry_by_path[graph_path] = entry
        self.graph_entries.append(entry)
        nodes = graph.get("nodes", [])
        if kind == "state_machine":
            anchors = [node for node in nodes if node_class(node) in FSM_STRUCTURAL_CLASSES]
        elif graph["_source_graph_suffix"] == self.root_suffix:
            anchors = [node for node in nodes if node_class(node) in ROOT_CLASSES]
        else:
            anchors = [node for node in nodes if node_class(node) in RESULT_CLASSES]
        if not anchors:
            self.record_issue(
                "source_graph_anchor_missing",
                source_graph=graph["_source_graph_suffix"],
                graph_kind=kind,
            )
        for node in sorted(anchors, key=lambda value: self.node_rank[node_identity(value)]):
            self.visit_node(node_identity(node), graph_path)

    def cache_producer(self, node_path):
        node = self.nodes[node_path]
        direct = node_property(node, "SaveCachedPoseNode")
        references = object_references(direct)
        if len(references) == 1 and references[0] in self.nodes:
            return references[0]
        cache_name = node_property(node, "NameOfCache")
        if not cache_name:
            return None
        candidates = [
            candidate_path
            for candidate_path, candidate in self.nodes.items()
            if node_class(candidate) in CACHE_SAVE_CLASSES
            and node_property(candidate, "CacheName") == cache_name
        ]
        if len(candidates) == 1:
            return candidates[0]
        return None

    def external_references(self, node_path):
        node = self.nodes[node_path]
        kind = EXTERNAL_NODE_KINDS.get(node_class(node))
        if kind is None:
            return
        references = sorted(
            {
                reference
                for property_value in node.get("properties", [])
                for reference in object_references(property_value.get("exported_text"))
            }
        )
        self.external_dependencies.append(
            {
                "source_node": node_path,
                "kind": kind,
                "references": references,
            }
        )

    def visit_function(self, name, parent_source_graph, source_node, call_site):
        candidates = self.function_graphs.get(name, [])
        dependency = {
            "source_node": source_node,
            "source_function": name,
            "call_site": call_site,
            "source_graph": None,
            "resolution": "external",
        }
        if len(candidates) == 0:
            if dependency not in self.function_dependencies:
                self.function_dependencies.append(dependency)
            return
        if len(candidates) != 1:
            self.record_issue(
                "source_function_graph_missing_or_ambiguous",
                source_node=source_node,
                source_function=name,
                call_site=call_site,
                candidate_count=len(candidates),
            )
            return
        graph = candidates[0]
        graph_path = graph["object_path"]
        dependency["source_graph"] = graph["_source_graph_suffix"]
        dependency["resolution"] = "graph"
        if dependency not in self.function_dependencies:
            self.function_dependencies.append(dependency)
        if graph_path in self.graph_entry_by_path:
            return
        entry = {
            "source_graph": graph["_source_graph_suffix"],
            "kind": "function",
            "class_path": graph["class_path"],
            "name": graph.get("name"),
            "index": graph.get("index"),
            "parent_source_graph": parent_source_graph,
            "reason": "lifecycle_function",
            "reachable_nodes": [node_identity(node) for node in graph.get("nodes", [])],
            "unreachable_nodes": [],
        }
        self.graph_entry_by_path[graph_path] = entry
        self.graph_entries.append(entry)

    def visit_node(self, node_path, graph_path):
        if node_path in self.visited_nodes:
            return
        node = self.nodes.get(node_path)
        if node is None:
            self.record_issue("source_node_missing", source_node=node_path)
            return
        self.visited_nodes.add(node_path)
        entry = self.graph_entry_by_path[graph_path]
        entry["reachable_nodes"].append(node_path)
        for upstream in self.incoming.get(node_path, []):
            self.visit_node(upstream, self.node_graph[upstream])
        class_path = node_class(node)
        if class_path in CACHE_USE_CLASSES:
            producer = self.cache_producer(node_path)
            if producer is None:
                self.record_issue("cached_pose_producer_missing", source_node=node_path)
            else:
                self.visit_node(producer, self.node_graph[producer])
        if class_path in STATE_MACHINE_CLASSES | FSM_STRUCTURAL_CLASSES:
            for child in self.graph_children_for_node(node_path):
                self.visit_graph(
                    child,
                    parent_source_graph=entry["source_graph"],
                    reason="state_machine" if class_path in STATE_MACHINE_CLASSES else "state_topology",
                )
        self.external_references(node_path)
        for call_site, function_name in animation_function_references(node):
            self.visit_function(
                function_name,
                parent_source_graph=entry["source_graph"],
                source_node=node_path,
                call_site=call_site,
            )

    def build(self):
        root_graph = self.graphs.get(self.root_suffix)
        if root_graph is None:
            raise ReachabilityError("root source graph is unavailable")
        self.visit_graph(root_graph)
        for entry in self.graph_entries:
            if entry["kind"] == "function":
                continue
            graph = self.graphs[entry["source_graph"]]
            unreachable = [
                node_identity(node)
                for node in graph.get("nodes", [])
                if node_identity(node) not in self.visited_nodes
                and node_class(node) != "/Script/UnrealEd.EdGraphNode_Comment"
            ]
            entry["unreachable_nodes"] = unreachable
        return {
            "format": FORMAT,
            "source_asset": self.source["asset_path"],
            "root_source_graph": self.root_suffix,
            "graphs": self.graph_entries,
            "external_dependencies": self.external_dependencies,
            "function_dependencies": self.function_dependencies,
            "issues": self.issues,
        }


def traverse(source, root_suffix):
    return RootTraversal(source, root_suffix).build()


def fixture_node(path, class_path, index, properties=None, title=None):
    value = {
        "index": index,
        "object_path": path,
        "class_path": class_path,
        "properties": properties or [],
        "pins": [],
    }
    if title is not None:
        value["title"] = title
    return value


def fixture_link(source, target):
    return {
        "source": {"node_object_path": source},
        "target": {"node_object_path": target},
    }


def fixture_source():
    asset = "/Asset"
    root = asset + ":AnimGraph"
    state_machine = root + ".StateMachine"
    fsm = state_machine + ".Locomotion"
    state = fsm + ".State"
    state_graph = state + ".Idle"
    root_node = root + ".Root"
    function = root + ".Function"
    cache_save = root + ".Save"
    linked = root + ".Linked"
    state_result = state_graph + ".Result"
    cache_use = state_graph + ".Use"
    return {
        "asset_path": asset,
        "graphs": [
            {
                "index": 0,
                "object_path": root,
                "class_path": "/Script/AnimGraph.AnimationGraph",
                "name": "AnimGraph",
                "nodes": [
                    fixture_node(root_node, "/Script/AnimGraph.AnimGraphNode_Root", 0),
                    fixture_node(
                        function,
                        "/Script/AnimGraph.AnimGraphNode_CallFunction",
                        1,
                        title="Refresh\nOn Update",
                    ),
                    fixture_node(state_machine, "/Script/AnimGraph.AnimGraphNode_StateMachine", 2),
                    fixture_node(
                        cache_save,
                        "/Script/AnimGraph.AnimGraphNode_SaveCachedPose",
                        3,
                        [{"name": "CacheName", "exported_text": "Grounded"}],
                    ),
                    fixture_node(
                        linked,
                        "/Script/AnimGraph.AnimGraphNode_LinkedAnimGraph",
                        4,
                        [{"name": "Node", "exported_text": "(InstanceClass=\"/Script/Engine.AnimBlueprintGeneratedClass'/Asset/Child.Child_C'\")"}],
                    ),
                ],
                "links": [
                    fixture_link(function, root_node),
                    fixture_link(state_machine, function),
                    fixture_link(linked, cache_save),
                ],
            },
            {
                "index": 1,
                "object_path": fsm,
                "class_path": "/Script/AnimGraph.AnimationStateMachineGraph",
                "name": "Locomotion",
                "nodes": [fixture_node(state, "/Script/AnimGraph.AnimStateNode", 0)],
                "links": [],
            },
            {
                "index": 2,
                "object_path": state_graph,
                "class_path": "/Script/AnimGraph.AnimationStateGraph",
                "name": "Idle",
                "nodes": [
                    fixture_node(state_result, "/Script/AnimGraph.AnimGraphNode_StateResult", 0),
                    fixture_node(
                        cache_use,
                        "/Script/AnimGraph.AnimGraphNode_UseCachedPose",
                        1,
                        [{"name": "NameOfCache", "exported_text": "Grounded"}],
                    ),
                ],
                "links": [fixture_link(cache_use, state_result)],
            },
            {
                "index": 3,
                "object_path": asset + ":Refresh",
                "class_path": "/Script/Engine.EdGraph",
                "name": "Refresh",
                "nodes": [
                    fixture_node(
                        asset + ":Refresh.Entry",
                        "/Script/BlueprintGraph.K2Node_FunctionEntry",
                        0,
                    )
                ],
            },
        ],
    }


def fixture_source_with_native_pin_links():
    source = fixture_source()
    graph = source["graphs"][0]
    graph.pop("links")
    nodes = {node["object_path"]: node for node in graph["nodes"]}
    nodes["/Asset:AnimGraph.Root"]["pins"] = [
        {
            "name": "Result",
            "direction": "Input",
            "linked_to": [
                {
                    "node_object_path": "/Asset:AnimGraph.Function",
                    "direction": "Output",
                }
            ],
        }
    ]
    nodes["/Asset:AnimGraph.Function"]["pins"] = [
        {
            "name": "Pose",
            "direction": "Input",
            "linked_to": [
                {
                    "node_object_path": "/Asset:AnimGraph.StateMachine",
                    "direction": "Output",
                }
            ],
        }
    ]
    nodes["/Asset:AnimGraph.Save"]["pins"] = [
        {
            "name": "Pose",
            "direction": "Input",
            "linked_to": [
                {
                    "node_object_path": "/Asset:AnimGraph.Linked",
                    "direction": "Output",
                }
            ],
        }
    ]
    return source


def run_self_test():
    result = traverse(fixture_source(), "AnimGraph")
    graphs = {entry["source_graph"]: entry for entry in result["graphs"]}
    root_nodes = graphs["AnimGraph"]["reachable_nodes"]
    assert "/Asset:AnimGraph.Root" in root_nodes
    assert "/Asset:AnimGraph.StateMachine" in root_nodes
    assert "/Asset:AnimGraph.Save" in root_nodes
    assert "/Asset:AnimGraph.Linked" in root_nodes
    assert graphs["AnimGraph.StateMachine.Locomotion.State.Idle"]["reachable_nodes"] == [
        "/Asset:AnimGraph.StateMachine.Locomotion.State.Idle.Result",
        "/Asset:AnimGraph.StateMachine.Locomotion.State.Idle.Use",
    ]
    assert result["external_dependencies"][0]["kind"] == "linked_anim_graph"
    assert result["function_dependencies"] == [
        {
            "source_node": "/Asset:AnimGraph.Function",
            "source_function": "Refresh",
            "call_site": "anim_node",
            "source_graph": "Refresh",
            "resolution": "graph",
        }
    ]
    assert any(entry["kind"] == "function" for entry in result["graphs"])
    assert not result["issues"]
    native_result = traverse(fixture_source_with_native_pin_links(), "AnimGraph")
    native_nodes = native_result["graphs"][0]["reachable_nodes"]
    assert "/Asset:AnimGraph.Function" in native_nodes
    assert "/Asset:AnimGraph.StateMachine" in native_nodes
    assert "/Asset:AnimGraph.Save" in native_nodes
    assert "/Asset:AnimGraph.Linked" in native_nodes
    assert not native_result["issues"]
    return {
        "root_pose_walks_upstream_links": "PASS",
        "state_machine_walks_state_graphs": "PASS",
        "cached_pose_walks_producer": "PASS",
        "linked_anim_dependency_recorded": "PASS",
        "lifecycle_function_graph_recorded": "PASS",
        "native_pin_links_walk_from_root": "PASS",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
