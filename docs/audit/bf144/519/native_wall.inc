        if ((*Case)->HasField(TEXT("wall_x_cm")))
        {
            double WallX;
            if (!(*Case)->HasTypedField<EJson::Number>(TEXT("wall_x_cm")) ||
                !(*Case)->TryGetNumberField(TEXT("wall_x_cm"), WallX) ||
                !FMath::IsFinite(WallX)) { return nullptr; }
            AActor* Wall = World->SpawnActor<AActor>();
            if (Wall == nullptr) { return nullptr; }
            UBoxComponent* Barrier = NewObject<UBoxComponent>(Wall);
            Wall->SetRootComponent(Barrier);
            Wall->AddInstanceComponent(Barrier);
            Barrier->SetMobility(EComponentMobility::Static);
            Barrier->SetBoxExtent(FVector(2, 100, 200));
            Barrier->SetWorldLocationAndRotation(FVector(SpawnXY.X + WallX, SpawnXY.Y, 100), FRotator(0, 45, 0));
            Barrier->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            Barrier->SetCollisionResponseToAllChannels(ECR_Block);
            Barrier->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            Barrier->RegisterComponent();
        }
