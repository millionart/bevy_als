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
        double AuditSlopeGapCm;
        if (!(*Case)->TryGetNumberField(TEXT("corner_slope_gap_cm"), AuditSlopeGapCm) ||
            (AuditSlopeGapCm != 0.15 && AuditSlopeGapCm != 0.188)) { return nullptr; }
        const FRotator SlopeRotation(-60, 0, 0);
        const FVector CapsuleSphere(SpawnXY.X + 200, SpawnXY.Y, 140);
        const FVector SlopeCenter = CapsuleSphere - (32 + 0.5 * AuditSlopeGapCm) * SlopeRotation.RotateVector(FVector::UpVector);
        if (!SpawnCornerBox(TEXT("AuditAirCornerWall"), FVector(SpawnXY.X + 235.1, SpawnXY.Y, 200), FRotator::ZeroRotator, FVector(5, 100, 150)) ||
            !SpawnCornerBox(TEXT("AuditAirCornerSecond"), FVector(SpawnXY.X + 200, SpawnXY.Y + 35.45, 200), FRotator::ZeroRotator, FVector(100, 5, 150)) ||
            !SpawnCornerBox(TEXT("AuditAirCornerSlope"), SlopeCenter, SlopeRotation, FVector(65, 100, 2)))
        {
            return nullptr;
        }
