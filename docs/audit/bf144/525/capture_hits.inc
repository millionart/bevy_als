            if (bObserveHits)
            {
                const auto CaptureFloorQuery = [&](const FVector& Location)
                {
                    FFindFloorResult Floor;
                    Movement->K2_FindFloor(Location, Floor);
                    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                    Value->SetBoolField(TEXT("walkable"), Floor.IsWalkableFloor());
                    Value->SetBoolField(TEXT("line_trace"), Floor.bLineTrace);
                    Value->SetNumberField(TEXT("floor_dist_cm"), Floor.GetDistanceToFloor());
                    Value->SetStringField(TEXT("component"), GetNameSafe(Floor.HitResult.GetComponent()));
                    return Value;
                };
                TArray<TSharedPtr<FJsonValue>> BlockingHits;
                for (const FHitResult& Hit : HitObserver->Hits)
                {
                    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                    Value->SetStringField(TEXT("component"), GetNameSafe(Hit.GetComponent()));
                    Value->SetBoolField(TEXT("penetrating"), Hit.bStartPenetrating);
                    Value->SetNumberField(TEXT("time"), Hit.Time);
                    Value->SetObjectField(TEXT("location_cm"), CaptureVector(Hit.Location));
                    Value->SetObjectField(TEXT("impact_point_cm"), CaptureVector(Hit.ImpactPoint));
                    Value->SetObjectField(TEXT("trace_start_cm"), CaptureVector(Hit.TraceStart));
                    Value->SetObjectField(TEXT("trace_end_cm"), CaptureVector(Hit.TraceEnd));
                    Value->SetObjectField(TEXT("normal"), CaptureVector(Hit.Normal));
                    Value->SetObjectField(TEXT("impact_normal"), CaptureVector(Hit.ImpactNormal));
                    Value->SetBoolField(TEXT("normals_equal"), Hit.Normal.Equals(Hit.ImpactNormal));
                    Value->SetBoolField(TEXT("edge_within_at_hit"),
                        Movement->IsWithinEdgeTolerance(Hit.Location, Hit.ImpactPoint,
                            Character->GetCapsuleComponent()->GetScaledCapsuleRadius()));
                    Value->SetBoolField(TEXT("walkable_hit"), Movement->IsWalkable(Hit));
                    Value->SetObjectField(TEXT("floor_at_hit"), CaptureFloorQuery(Hit.Location));
                    BlockingHits.Add(MakeShared<FJsonValueObject>(Value));
                }
                Row->SetArrayField(TEXT("blocking_hits"), BlockingHits);
                Row->SetObjectField(TEXT("floor_after_tick_query"), CaptureFloorQuery(Character->GetActorLocation()));
            }
