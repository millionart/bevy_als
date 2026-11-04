                const auto CaptureActualFloor = [&](const FFindFloorResult& Actual)
                {
                    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                    Value->SetBoolField(TEXT("blocking"), Actual.bBlockingHit);
                    Value->SetBoolField(TEXT("walkable"), Actual.IsWalkableFloor());
                    Value->SetBoolField(TEXT("line_trace"), Actual.bLineTrace);
                    Value->SetNumberField(TEXT("distance_cm"), Actual.GetDistanceToFloor());
                    Value->SetObjectField(TEXT("trace_start_cm"), CaptureVector(Actual.HitResult.TraceStart));
                    Value->SetObjectField(TEXT("trace_end_cm"), CaptureVector(Actual.HitResult.TraceEnd));
                    Value->SetObjectField(TEXT("hit"), CaptureLandingHit(Actual.HitResult));
                    return Value;
                };
                Row->SetObjectField(TEXT("actual_current_floor"), CaptureActualFloor(Movement->CurrentFloor));
