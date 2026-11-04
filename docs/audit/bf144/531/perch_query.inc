                    const float MaxFloor = FMath::Max(UCharacterMovementComponent::MAX_FLOOR_DIST,
                        Movement->MaxStepHeight + (Movement->IsMovingOnGround() ?
                            UCharacterMovementComponent::MAX_FLOOR_DIST + UE_KINDA_SMALL_NUMBER :
                            -UCharacterMovementComponent::MAX_FLOOR_DIST));
                    const auto CaptureFloorParts = [&](const FFindFloorResult& Part)
                    {
                        TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
                        Result->SetBoolField(TEXT("blocking"), Part.bBlockingHit);
                        Result->SetBoolField(TEXT("walkable"), Part.IsWalkableFloor());
                        Result->SetBoolField(TEXT("line_trace"), Part.bLineTrace);
                        Result->SetNumberField(TEXT("sweep_dist_cm"), Part.FloorDist);
                        Result->SetNumberField(TEXT("line_dist_cm"), Part.LineDist);
                        Result->SetObjectField(TEXT("hit"), CaptureLandingHit(Part.HitResult));
                        return Result;
                    };
                    FFindFloorResult Broad;
                    Movement->K2_ComputeFloorDist(Location, MaxFloor, MaxFloor,
                        Character->GetCapsuleComponent()->GetScaledCapsuleRadius(), Broad);
                    Value->SetObjectField(TEXT("broad"), CaptureFloorParts(Broad));
                    Value->SetObjectField(TEXT("merged"), CaptureFloorParts(Floor));
                    const bool bTryPerch = Broad.bBlockingHit && !Broad.bLineTrace &&
                        Movement->ShouldComputePerchResult(Broad.HitResult, true);
                    Value->SetBoolField(TEXT("perch_requested"), bTryPerch);
                    if (bTryPerch)
                    {
                        FFindFloorResult Perch;
                        const float PerchRadius = Movement->GetValidPerchRadius();
                        const float MaxPerch = MaxFloor + (Movement->IsMovingOnGround() ?
                            FMath::Max(0.f, Movement->PerchAdditionalHeight) : 0.f);
                        const bool bValidPerch = Movement->ComputePerchResult(PerchRadius, Broad.HitResult, MaxPerch, Perch);
                        Value->SetBoolField(TEXT("perch_valid"), bValidPerch);
                        Value->SetNumberField(TEXT("perch_radius_cm"), PerchRadius);
                        Value->SetObjectField(TEXT("perch"), CaptureFloorParts(Perch));
                    }
