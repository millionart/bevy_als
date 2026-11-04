        if ((*Case)->HasField(TEXT("landing_deck_top_cm")))
        {
            double Top;
            if (!(*Case)->TryGetNumberField(TEXT("landing_deck_top_cm"), Top) || !FMath::IsFinite(Top)) { return nullptr; }
            AActor* Deck = World->SpawnActor<AActor>();
            if (Deck == nullptr) { return nullptr; }
            UBoxComponent* DeckBox = NewObject<UBoxComponent>(Deck);
            Deck->SetRootComponent(DeckBox);
            Deck->AddInstanceComponent(DeckBox);
            DeckBox->SetMobility(EComponentMobility::Static);
            DeckBox->SetBoxExtent(FVector(100, 65, 10));
            DeckBox->SetWorldLocation(FVector(SpawnXY.X + 230, SpawnXY.Y - 30, Top - 10));
            DeckBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            DeckBox->SetCollisionResponseToAllChannels(ECR_Block);
            DeckBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            DeckBox->RegisterComponent();
        }
