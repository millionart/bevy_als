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
        const FRotator SlopeRotation(0, 10, -60);
        const FVector SlopeCenter = FVector(SpawnXY.X + 200, SpawnXY.Y, 140) - 38.5 * SlopeRotation.RotateVector(FVector::UpVector);
        double AuditLandingTop;
        if (!(*Case)->TryGetNumberField(TEXT("third_floor_top_cm"), AuditLandingTop) ||
            (AuditLandingTop != 104 && AuditLandingTop != 106.5)) { return nullptr; }
        if (!SpawnCornerBox(TEXT("AuditAirCornerWall"), FVector(SpawnXY.X + 238, SpawnXY.Y, 200), FRotator::ZeroRotator, FVector(5, 100, 150)) ||
            !SpawnCornerBox(TEXT("AuditAirCornerSlope"), SlopeCenter, SlopeRotation, FVector(100, 60, 2)) ||
            !SpawnCornerBox(TEXT("AuditThirdFloor"), FVector(SpawnXY.X + 200, SpawnXY.Y, AuditLandingTop - 10), FRotator::ZeroRotator, FVector(100, 100, 10)))
        {
            return nullptr;
        }
