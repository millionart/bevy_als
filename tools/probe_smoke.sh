#!/bin/bash
set -u
cd "$(dirname "$0")/.."

echo "== Test 1: GameplayTags.ListTags =="
python tools/mcp_probe.py call GameplayTagsToolset.GameplayTagsToolset ListTags '{}' 2>&1 | head -15

echo
echo "== Test 2: AssetTools.find_assets (SkeletalMesh) =="
python tools/mcp_probe.py call editor_toolset.toolsets.asset.AssetTools find_assets '{"folder_path":"/Game","name":"","recursive":true}' 2>&1 | head -40

echo
echo "== done =="
