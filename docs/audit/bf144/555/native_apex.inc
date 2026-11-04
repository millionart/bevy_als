        bool bSplit, bRoof;
        if (!(*Case)->TryGetBoolField(TEXT("force_apex_split"), bSplit) ||
            !(*Case)->TryGetBoolField(TEXT("apex_roof"), bRoof)) { return nullptr; }
        IConsoleVariable* ApexSplit = IConsoleManager::Get().FindConsoleVariable(TEXT("p.ForceJumpPeakSubstep"));
        if (ApexSplit == nullptr) { return nullptr; }
        const int32 SavedApexSplit = ApexSplit->GetInt();
        const EConsoleVariableFlags SavedApexFlags = static_cast<EConsoleVariableFlags>(ApexSplit->GetFlags() & ECVF_SetByMask);
        ApexSplit->Set(bSplit ? 1 : 0, ECVF_SetByCode);
        ON_SCOPE_EXIT {
            ApexSplit->Set(SavedApexSplit, ECVF_SetByCode);
            ApexSplit->ClearFlags(ECVF_SetByMask);
            ApexSplit->SetFlags(SavedApexFlags);
        };
        if (ApexSplit->GetInt() != (bSplit ? 1 : 0)) { return nullptr; }
        if (bRoof)
        {
            AActor* RoofActor = World->SpawnActor<AActor>();
            if (RoofActor == nullptr) { return nullptr; }
            UBoxComponent* ApexRoof = NewObject<UBoxComponent>(RoofActor, TEXT("AuditApexRoof"));
            RoofActor->SetRootComponent(ApexRoof);
            RoofActor->AddInstanceComponent(ApexRoof);
            ApexRoof->SetMobility(EComponentMobility::Static);
            ApexRoof->SetBoxExtent(FVector(50, 50, 5));
            ApexRoof->SetWorldLocation(FVector(SpawnXY.X + 200, SpawnXY.Y, 295.15));
            ApexRoof->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            ApexRoof->SetCollisionResponseToAllChannels(ECR_Block);
            ApexRoof->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            ApexRoof->RegisterComponent();
        }
