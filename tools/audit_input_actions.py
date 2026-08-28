import json

from mcp_probe import ensure_session, extract_text, post


ACTIONS = {
    "Aim": "/ALS/ALS/Data/Input/IA_Als_Aim.IA_Als_Aim",
    "Crouch": "/ALS/ALS/Data/Input/IA_Als_Crouch.IA_Als_Crouch",
    "Jump": "/ALS/ALS/Data/Input/IA_Als_Jump.IA_Als_Jump",
    "Look": "/ALS/ALS/Data/Input/IA_Als_Look.IA_Als_Look",
    "LookMouse": "/ALS/ALS/Data/Input/IA_Als_LookMouse.IA_Als_LookMouse",
    "Move": "/ALS/ALS/Data/Input/IA_Als_Move.IA_Als_Move",
    "Ragdoll": "/ALS/ALS/Data/Input/IA_Als_Ragdoll.IA_Als_Ragdoll",
    "Roll": "/ALS/ALS/Data/Input/IA_Als_Roll.IA_Als_Roll",
    "RotationMode": "/ALS/ALS/Data/Input/IA_Als_RotationMode.IA_Als_RotationMode",
    "Sprint": "/ALS/ALS/Data/Input/IA_Als_Sprint.IA_Als_Sprint",
    "SwitchShoulder": "/ALS/ALS/Data/Input/IA_Als_SwitchShoulder.IA_Als_SwitchShoulder",
    "ViewMode": "/ALS/ALS/Data/Input/IA_Als_ViewMode.IA_Als_ViewMode",
    "Walk": "/ALS/ALS/Data/Input/IA_Als_Walk.IA_Als_Walk",
    "NextOverlayMode": "/ALS/ALSExtras/Data/Input/IA_Als_NextOverlayMode.IA_Als_NextOverlayMode",
    "OverlayModeMenu": "/ALS/ALSExtras/Data/Input/IA_Als_OverlayModeMenu.IA_Als_OverlayModeMenu",
    "PreviousOverlayMode": "/ALS/ALSExtras/Data/Input/IA_Als_PreviousOverlayMode.IA_Als_PreviousOverlayMode",
    "Slomo": "/ALS/ALSExtras/Data/Input/IA_Als_Slomo.IA_Als_Slomo",
    "ToggleUI": "/ALS/ALSExtras/Data/Input/IA_Als_ToggleUI.IA_Als_ToggleUI",
}

TRIGGER_FIELDS = [
    "holdTimeThreshold",
    "tapReleaseTimeThreshold",
    "bIsOneShot",
    "actuationThreshold",
    "bAffectedByTimeDilation",
]


def call(sid, tool_name, arguments):
    body, error = post(
        {
            "jsonrpc": "2.0",
            "id": 41,
            "method": "tools/call",
            "params": {
                "name": "call_tool",
                "arguments": {
                    "toolset_name": "editor_toolset.toolsets.object.ObjectTools",
                    "tool_name": tool_name,
                    "arguments": arguments,
                },
            },
        },
        sid,
    )
    if body is None:
        raise RuntimeError(error)
    result = json.loads(extract_text(body))
    return json.loads(result["returnValue"])


def inspect_action(sid, path):
    action = call(sid, "get_properties", {"instance": {"refPath": path}, "properties": ["triggers", "valueType"]})
    triggers = []
    for entry in action.get("triggers", []):
        trigger_path = entry["refPath"]
        schema = call(sid, "list_properties", {"instance": {"refPath": trigger_path}})
        fields = [field for field in TRIGGER_FIELDS if field in schema]
        values = call(sid, "get_properties", {"instance": {"refPath": trigger_path}, "properties": fields})
        triggers.append({"path": trigger_path, "values": values})
    return {"valueType": action.get("valueType"), "triggers": triggers}


def main():
    sid = ensure_session()
    output = {}
    for name, path in ACTIONS.items():
        try:
            output[name] = inspect_action(sid, path)
        except Exception as error:
            output[name] = {"error": str(error)}
    print(json.dumps(output, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
