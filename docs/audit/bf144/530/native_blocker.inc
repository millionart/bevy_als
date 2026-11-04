        if ((*Case)->HasField(TEXT("floor_blocker_cm")))
        {
            const TArray<TSharedPtr<FJsonValue>>* Center = nullptr;
            if (!(*Case)->TryGetArrayField(TEXT("floor_blocker_cm"), Center) || Center->Num() != 3) { return nullptr; }
            for (const TSharedPtr<FJsonValue>& Coordinate : *Center)
            {
                if (Coordinate->Type != EJson::Number || !FMath::IsFinite(Coordinate->AsNumber())) { return nullptr; }
            }
            AActor* Blocker = World->SpawnActor<AActor>();
            if (Blocker == nullptr) { return nullptr; }
            UBoxComponent* BlockerBox = NewObject<UBoxComponent>(Blocker, TEXT("AuditFloorBlocker"));
            Blocker->SetRootComponent(BlockerBox);
            Blocker->AddInstanceComponent(BlockerBox);
            BlockerBox->SetMobility(EComponentMobility::Static);
            BlockerBox->SetBoxExtent(FVector(7, 80, 1.5));
            BlockerBox->SetWorldLocationAndRotation(
                FVector(SpawnXY.X + (*Center)[0]->AsNumber(), SpawnXY.Y + (*Center)[1]->AsNumber(),
                    (*Center)[2]->AsNumber()), FRotator(-45, 45, 0));
            BlockerBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            BlockerBox->SetCollisionResponseToAllChannels(ECR_Block);
            BlockerBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            BlockerBox->RegisterComponent();
        }
