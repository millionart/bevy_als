                        float PawnRadius, PawnHalfHeight;
                        Character->GetCapsuleComponent()->GetScaledCapsuleSize(PawnRadius, PawnHalfHeight);
                        const FVector NarrowStart = Movement->bUseFlatBaseForFloorChecks ? Broad.HitResult.TraceStart : Broad.HitResult.Location;
                        const float AboveBase = FMath::Max<float>(0.f,
                            Movement->GetGravitySpaceZ(Broad.HitResult.ImpactPoint - NarrowStart) + PawnHalfHeight);
                        FFindFloorResult NarrowBeforeLimit;
                        Movement->K2_ComputeFloorDist(NarrowStart, FMath::Max(0.f, MaxPerch - AboveBase),
                            MaxPerch + PawnRadius, PerchRadius, NarrowBeforeLimit);
                        Value->SetNumberField(TEXT("above_base_cm"), AboveBase);
                        Value->SetNumberField(TEXT("max_perch_cm"), MaxPerch);
                        Value->SetNumberField(TEXT("narrow_total_cm"), AboveBase + NarrowBeforeLimit.FloorDist);
                        Value->SetObjectField(TEXT("narrow_before_limit"), CaptureFloorParts(NarrowBeforeLimit));
