        double RoofBottomCm;
        if (!(*Case)->TryGetNumberField(TEXT("step_roof_bottom_cm"), RoofBottomCm) ||
            (RoofBottomCm != 225.6 && RoofBottomCm != 228)) { return nullptr; }
        const auto SpawnAuditBox = [&](const TCHAR* Name, const FVector& Center, const FVector& Extent)
        {
            AActor* Actor = World->SpawnActor<AActor>();
            if (Actor == nullptr) { return false; }
            UBoxComponent* Part = NewObject<UBoxComponent>(Actor, FName(Name));
            Actor->SetRootComponent(Part);
            Actor->AddInstanceComponent(Part);
            Part->SetMobility(EComponentMobility::Static);
            Part->SetBoxExtent(Extent);
            Part->SetWorldLocation(Center);
            Part->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            Part->SetCollisionResponseToAllChannels(ECR_Block);
            Part->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            Part->RegisterComponent();
            return true;
        };
        if (!SpawnAuditBox(TEXT("AuditStepLimit"), FVector(SpawnXY.X + 160, SpawnXY.Y, 22), FVector(25, 100, 22)) ||
            !SpawnAuditBox(TEXT("AuditStepRoof"), FVector(SpawnXY.X + 115, SpawnXY.Y, RoofBottomCm + 5), FVector(65, 100, 5)))
        {
            return nullptr;
        }
