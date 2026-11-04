                    if (Event.Previous == MOVE_Walking && Event.Current == MOVE_Falling)
                    {
                        const float DownHeight = Movement->MaxStepHeight + 2.f * UCharacterMovementComponent::MAX_FLOOR_DIST;
                        const FVector End = Event.Location;
                        const FVector Start = End - Movement->GetGravityDirection() * DownHeight;
                        UCapsuleComponent* Capsule = Character->GetCapsuleComponent();
                        FComponentQueryParams Query(SCENE_QUERY_STAT(MoveComponent), Character);
                        FCollisionResponseParams Response;
                        Capsule->InitSweepCollisionParams(Query, Response);
                        Query.bIgnoreTouches = true;
                        TArray<FHitResult> Hits;
                        const bool bBlocking = World->ComponentSweepMulti(Hits, Capsule, Start, End,
                            Capsule->GetComponentQuat(), Query);
                        TSharedRef<FJsonObject> DownReplay = MakeShared<FJsonObject>();
                        DownReplay->SetObjectField(TEXT("start_cm"), CaptureVector(Start));
                        DownReplay->SetObjectField(TEXT("end_cm"), CaptureVector(End));
                        DownReplay->SetBoolField(TEXT("blocking"), bBlocking);
                        DownReplay->SetNumberField(TEXT("hit_count"), Hits.Num());
                        Change->SetObjectField(TEXT("down_replay"), DownReplay);
                    }
