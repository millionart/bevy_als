#[derive(Clone,Copy)]
pub struct HermiteKey {pub time:f32,pub value:f32,pub arrive_tangent:f32,pub leave_tangent:f32}
#[derive(Clone)]
pub struct HermiteCurve {pub keys:Vec<HermiteKey>}
impl HermiteCurve {
    pub fn sample(&self, time: f32) -> f32 {
        let Some(first) = self.keys.first() else {
            return 0.0;
        };
        let Some(last) = self.keys.last() else {
            return 0.0;
        };
        if time <= first.time {
            return first.value;
        }
        if time >= last.time {
            return last.value;
        }
        let upper = self.keys.partition_point(|key| key.time <= time);
        let start = self.keys[upper - 1];
        let end = self.keys[upper];
        let duration = (end.time - start.time).max(f32::EPSILON);
        let normalized = ((time - start.time) / duration).clamp(0.0, 1.0);
        let normalized_squared = normalized * normalized;
        let normalized_cubed = normalized_squared * normalized;
        let h00 = 2.0 * normalized_cubed - 3.0 * normalized_squared + 1.0;
        let h10 = normalized_cubed - 2.0 * normalized_squared + normalized;
        let h01 = -2.0 * normalized_cubed + 3.0 * normalized_squared;
        let h11 = normalized_cubed - normalized_squared;
        h00 * start.value
            + h10 * duration * start.leave_tangent
            + h01 * end.value
            + h11 * duration * end.arrive_tangent
    }
}

fn main(){let curves=[HermiteCurve { keys: vec![HermiteKey { time:0.0, value:20.0, arrive_tangent:0.0, leave_tangent:0.0 },HermiteKey { time:1.0, value:20.0, arrive_tangent:0.0, leave_tangent:0.0 },HermiteKey { time:2.0, value:20.0, arrive_tangent:0.0, leave_tangent:-36.423_724 },HermiteKey { time:3.0, value:7.5, arrive_tangent:0.0, leave_tangent:0.0 }] },HermiteCurve { keys: vec![HermiteKey { time:0.0, value:15.0, arrive_tangent:0.0, leave_tangent:0.0 },HermiteKey { time:1.0, value:15.0, arrive_tangent:0.0, leave_tangent:-2.5 },HermiteKey { time:2.0, value:12.5, arrive_tangent:-2.5, leave_tangent:-7.5 },HermiteKey { time:3.0, value:5.0, arrive_tangent:-7.5, leave_tangent:0.0 }] },HermiteCurve { keys: vec![HermiteKey { time:0.0, value:5.0, arrive_tangent:0.0, leave_tangent:0.0 },HermiteKey { time:1.0, value:5.0, arrive_tangent:0.0, leave_tangent:-1.0 },HermiteKey { time:2.0, value:4.0, arrive_tangent:-1.0, leave_tangent:-3.5 },HermiteKey { time:3.0, value:0.5, arrive_tangent:-3.5, leave_tangent:0.0 }] }];let mut corrected=curves[0].clone();corrected.keys[2].leave_tangent=-3642.3720703125f32/100.0;let times=[0.0f32,0.125f32,0.25f32,0.375f32,0.5f32,0.625f32,0.75f32,0.875f32,1.0f32,1.1131258010864258f32,1.125f32,1.25f32,1.375f32,1.5f32,1.625f32,1.75f32,1.7916673421859741f32,1.875f32,2.0f32,2.125f32,2.25f32,2.375f32,2.5f32,2.625f32,2.75f32,2.875f32,3.0f32];let mut output=Vec::new();for t in times{for c in &curves{output.extend(c.sample(t).to_le_bytes());}output.extend(corrected.sample(t).to_le_bytes());}std::fs::write(std::env::args().nth(1).unwrap(),output).unwrap();println!("Captured unchanged target Hermite method excerpt and source-key-only control");}
