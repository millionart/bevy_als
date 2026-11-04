                if (Rows.Num() == 40 && !HitObserver->Hits.IsEmpty())
                {
                    const FHitResult& First = HitObserver->Hits[0];
                    const FVector GravityDir = Movement->GetGravityDirection();
                    const float UpHeight = FMath::Max(0.f, Movement->MaxStepHeight -
                        static_cast<float>(MovementBefore->GetNumberField(TEXT("floor_distance_cm"))));
                    const float DownHeight = Movement->MaxStepHeight + 2.f * UCharacterMovementComponent::MAX_FLOOR_DIST;
                    const FVector Start = First.TraceEnd - GravityDir * UpHeight;
                    const FVector End = Start + GravityDir * DownHeight;
                    UCapsuleComponent* Capsule = Character->GetCapsuleComponent();
                    FComponentQueryParams Query(SCENE_QUERY_STAT(MoveComponent), Character);
                    FCollisionResponseParams Response;
                    Capsule->InitSweepCollisionParams(Query, Response);
                    Query.bIgnoreTouches = true;
                    TArray<FHitResult> Hits;
                    World->ComponentSweepMulti(Hits, Capsule, Start, End, Capsule->GetComponentQuat(), Query);
                    const FHitResult* Blocking = Hits.FindByPredicate([](const FHitResult& Hit) { return Hit.bBlockingHit; });
                    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
                    Probe->SetObjectField(TEXT("start_cm"), CaptureVector(Start));
                    Probe->SetObjectField(TEXT("end_cm"), CaptureVector(End));
                    if (Blocking != nullptr)
                    {
                        Probe->SetObjectField(TEXT("hit"), CaptureLandingHit(*Blocking));
                        Probe->SetNumberField(TEXT("direction_dot"),
                            FVector::DotProduct(First.TraceEnd - First.TraceStart, Blocking->ImpactNormal));
                        Probe->SetBoolField(TEXT("above_old"),
                            Movement->GetGravitySpaceZ(Blocking->Location) > Movement->GetGravitySpaceZ(First.TraceStart));
                    }
                    Row->SetObjectField(TEXT("candidate_down_replay"), Probe);
                }
