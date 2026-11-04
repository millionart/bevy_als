        if ((*Case)->HasField(TEXT("step_corner_second")))
        {
            bool bSecond;
            if (!(*Case)->TryGetBoolField(TEXT("step_corner_second"), bSecond)) { return nullptr; }
            const auto SpawnWall = [&](const TCHAR* Name, double XAtHit, double Yaw)
            {
                AActor* Actor = World->SpawnActor<AActor>();
                if (Actor == nullptr) { return false; }
                UBoxComponent* Box = NewObject<UBoxComponent>(Actor, FName(Name));
                Actor->SetRootComponent(Box);
                Actor->AddInstanceComponent(Box);
                Box->SetMobility(EComponentMobility::Static);
                Box->SetBoxExtent(FVector(2, 50, 150));
                const FRotator Rotation(0, Yaw, 0);
                const FVector Center = FVector(SpawnXY.X + XAtHit, SpawnXY.Y, 150) + Rotation.Vector() * 32;
                Box->SetWorldLocationAndRotation(Center, Rotation);
                Box->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
                Box->SetCollisionResponseToAllChannels(ECR_Block);
                Box->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
                Box->RegisterComponent();
                return true;
            };
            if (!SpawnWall(TEXT("AuditStepWallFirst"), 113, -45) ||
                (bSecond && !SpawnWall(TEXT("AuditStepWallSecond"), 116, -20)))
            {
                return nullptr;
            }
        }
