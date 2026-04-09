# Vulkan Layers for NV_ENGINE

This folder is the project-local location for Vulkan layer manifests and binaries.

## How to use

1. Place the Vulkan layer manifest JSON and the corresponding DLL into this folder.
2. In `.vscode/tasks.json`, the built-in task uses:
   - `VK_LAYER_PATH=${workspaceFolder}/VulkanLayers`
   - `VK_INSTANCE_LAYERS=VK_LAYER_NV2_ENGINE_sample`

3. Update `VK_INSTANCE_LAYERS` in `.vscode/tasks.json` to the actual layer name if different.
4. Run the task from VS Code: `Terminal > Run Task... > Run NV2 Engine with Vulkan Layer`.

## Notes

- I cannot install system-level Vulkan layers from inside this code workspace.
- You must still install or provide the actual Vulkan layer library / manifest files.
- The task pins the project to a workspace-local layer path so the engine uses it when launched from VS Code.
