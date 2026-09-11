using UnrealBuildTool;

public class AlsGraphSourceReader : ModuleRules
{
    public AlsGraphSourceReader(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;

        PublicDependencyModuleNames.AddRange(new[]
        {
            "Core",
            "CoreUObject",
            "Engine",
            "PhysicsCore"
        });

        PrivateDependencyModuleNames.AddRange(new[]
        {
            "UnrealEd",
            "BlueprintGraph",
            "Kismet",
            "AnimGraph",
            "ControlRig",
            "RigVM",
            "RigVMDeveloper",
            "Json",
            "JsonUtilities"
        });
    }
}
