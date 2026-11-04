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
        double WallYaw;
        if (!(*Case)->TryGetNumberField(TEXT("repeated_wall_yaw"), WallYaw) || WallYaw < 0)
        {
            return nullptr;
        }
        const FVector Origin(SpawnXY.X + 200, SpawnXY.Y, 200);
        if (!SpawnCornerBox(TEXT("AuditNudgeFirst"), Origin + FVector(35.08, 0, 0),
            FRotator::ZeroRotator, FVector(5, 80, 150))) { return nullptr; }
        if (WallYaw > 0)
        {
            const FRotator WallRotation(0, WallYaw, 0);
            const FVector Normal = WallRotation.RotateVector(FVector(-1, 0, 0));
            const double Gap = -14 * Normal.Y;
            if (!SpawnCornerBox(TEXT("AuditNudgeSecond"), Origin - (35 + Gap) * Normal,
                WallRotation, FVector(5, 80, 150))) { return nullptr; }
        }
