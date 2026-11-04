        double CeilingBottom;
        if (!(*Case)->TryGetNumberField(TEXT("step_ceiling_bottom_cm"), CeilingBottom) ||
            (CeilingBottom != 207 && CeilingBottom != 204)) { return nullptr; }
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
        if (!SpawnAuditBox(TEXT("AuditStepCeiling"), FVector(SpawnXY.X + 35, SpawnXY.Y, CeilingBottom + 5), FVector(50, 50, 5)) ||
            !SpawnAuditBox(TEXT("AuditStepEdge"), FVector(SpawnXY.X + 145, SpawnXY.Y, 21.5), FVector(4, 50, 21.5)))
        {
            return nullptr;
        }
