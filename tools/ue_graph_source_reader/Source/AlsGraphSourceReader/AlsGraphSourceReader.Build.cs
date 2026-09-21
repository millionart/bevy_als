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
            "ImageCore",
            "BlueprintGraph",
            "Kismet",
            "AnimGraph",
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
