pub fn unscaled_delta_seconds(
    previous_global_time_dilation: &mut f32,
    virtual_delta_seconds: f32,
    real_delta_seconds: f32,
    effective_global_time_dilation: f32,
    custom_time_dilation: f32,
) -> f32 {
    let time_dilation = *previous_global_time_dilation * custom_time_dilation;
    let delta_seconds = if time_dilation > 1e-8 {
        virtual_delta_seconds / time_dilation
    } else {
        real_delta_seconds
    };
    *previous_global_time_dilation = effective_global_time_dilation;
    delta_seconds.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_unscaled_delta_from_global_and_custom_dilation() {
        let mut previous_global_time_dilation = 0.2;
        let delta_seconds =
            unscaled_delta_seconds(&mut previous_global_time_dilation, 0.01, 0.1, 0.2, 0.5);
        let passed = (delta_seconds - 0.1).abs() <= 1e-6
            && (previous_global_time_dilation - 0.2).abs() <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.57][VERDICT] delta={delta_seconds:.6} previous_global={previous_global_time_dilation:.3} -> {verdict}",
        );
        assert!(passed);
        let mut previous = 1e-7;
        assert!((unscaled_delta_seconds(&mut previous, 1e-8, 0.2, 1.0, 1.0) - 0.1).abs() < 1e-6);
        let mut previous = 1e-8;
        assert_eq!(
            unscaled_delta_seconds(&mut previous, 1e-8, 0.2, 1.0, 1.0),
            0.2
        );
    }
}
