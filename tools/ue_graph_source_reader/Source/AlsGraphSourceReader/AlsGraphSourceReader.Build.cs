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
            "Chaos",
            "ImageCore",
            "BlueprintGraph",
            "Kismet",
            "AnimGraph",
            "AnimGraphRuntime",
            "ControlRig",
            "RigVM",
            "RigVMDeveloper",
            "Json",
            "JsonUtilities",
            "SlateCore",
            "SlateNullRenderer"
        });
    }
}
