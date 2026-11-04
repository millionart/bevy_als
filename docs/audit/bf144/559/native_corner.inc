        const auto SpawnCornerBox = [&](const TCHAR* Name, const FVector& Center, const FRotator& Rotation, const FVector& Extent)
        {
            AActor* Actor = World->SpawnActor<AActor>();
            if (Actor == nullptr) { return false; }
            UBoxComponent* Part = NewObject<UBoxComponent>(Actor, FName(Name));
            Actor->SetRootComponent(Part);
            Actor->AddInstanceComponent(Part);
            Part->SetMobility(EComponentMobility::Static);
            Part->SetBoxExtent(Extent);
            Part->SetWorldLocationAndRotation(Center, Rotation);
            Part->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            Part->SetCollisionResponseToAllChannels(ECR_Block);
            Part->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            Part->RegisterComponent();
            return true;
        };
        double AuditFloorTop;
        if (!(*Case)->TryGetNumberField(TEXT("corner_floor_top_cm"), AuditFloorTop) ||
            (AuditFloorTop != 108.03 && AuditFloorTop != 107.7)) { return nullptr; }
        if (!SpawnCornerBox(TEXT("AuditAirCornerWall"), FVector(SpawnXY.X + 238, SpawnXY.Y, 200), FRotator::ZeroRotator, FVector(5, 100, 150)) ||
            !SpawnCornerBox(TEXT("AuditAirCornerSecond"), FVector(SpawnXY.X + 200, SpawnXY.Y + 40.5, 200), FRotator::ZeroRotator, FVector(100, 5, 150)) ||
            !SpawnCornerBox(TEXT("AuditAirCornerFloor"), FVector(SpawnXY.X + 200, SpawnXY.Y, AuditFloorTop - 10), FRotator::ZeroRotator, FVector(100, 100, 10)))
        {
            return nullptr;
        }
