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
        double WallGap;
        if (!(*Case)->TryGetNumberField(TEXT("virtual_ditch_wall_gap_cm"), WallGap) || WallGap < 0)
        {
            return nullptr;
        }
        const FVector Sphere(SpawnXY.X + 200, SpawnXY.Y, 140);
        const FRotator SlopeRotation(-40, 0, 0);
        const FVector SlopeCenter = Sphere - 31.1 * SlopeRotation.RotateVector(FVector::UpVector);
        if (!SpawnCornerBox(TEXT("AuditDitchLedge"), SlopeCenter, SlopeRotation, FVector(3, 30, 1)) ||
            (WallGap > 0 && !SpawnCornerBox(TEXT("AuditDitchWall"),
                FVector(SpawnXY.X + 235 + WallGap, SpawnXY.Y, 200), FRotator::ZeroRotator, FVector(5, 100, 150))))
        {
            return nullptr;
        }
