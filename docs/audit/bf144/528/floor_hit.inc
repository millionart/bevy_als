                const auto CaptureLandingHit = [&](const FHitResult& Hit)
                {
                    float Radius, HalfHeight;
                    Character->GetCapsuleComponent()->GetScaledCapsuleSize(Radius, HalfHeight);
                    const double LowerHemisphere = Movement->GetGravitySpaceZ(Hit.Location) - HalfHeight + Radius;
                    const double ImpactZ = Movement->GetGravitySpaceZ(Hit.ImpactPoint);
                    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                    Value->SetStringField(TEXT("component"), GetNameSafe(Hit.GetComponent()));
                    Value->SetBoolField(TEXT("blocking"), Hit.bBlockingHit);
                    Value->SetBoolField(TEXT("penetrating"), Hit.bStartPenetrating);
                    Value->SetNumberField(TEXT("time"), Hit.Time);
                    Value->SetObjectField(TEXT("location_cm"), CaptureVector(Hit.Location));
                    Value->SetObjectField(TEXT("impact_point_cm"), CaptureVector(Hit.ImpactPoint));
                    Value->SetObjectField(TEXT("normal"), CaptureVector(Hit.Normal));
                    Value->SetObjectField(TEXT("impact_normal"), CaptureVector(Hit.ImpactNormal));
                    Value->SetNumberField(TEXT("lower_hemisphere_cm"), LowerHemisphere);
                    Value->SetNumberField(TEXT("impact_height_cm"), ImpactZ);
                    Value->SetBoolField(TEXT("below_lower_hemisphere"), ImpactZ < LowerHemisphere);
                    Value->SetBoolField(TEXT("edge_within"), Movement->IsWithinEdgeTolerance(Hit.Location, Hit.ImpactPoint, Radius));
                    Value->SetBoolField(TEXT("walkable"), Movement->IsWalkable(Hit));
                    return Value;
                };
