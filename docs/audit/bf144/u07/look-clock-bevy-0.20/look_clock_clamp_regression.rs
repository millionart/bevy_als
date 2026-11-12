use bevy::prelude::*;
use bevy::time::{Real, Virtual, update_virtual_time};
use std::time::Duration;

#[test]
fn clamped_view_time_is_divided_by_the_configured_dilation() {
    let mut real = Time::<Real>::default();
    real.advance_by(Duration::from_millis(100));
    let mut virtual_time = Time::<Virtual>::from_max_delta(Duration::from_micros(7500));
    virtual_time.set_relative_speed(0.15);
    update_virtual_time(&mut Time::<()>::default(), &mut virtual_time, &real);
    let previous = virtual_time.delta_secs() / virtual_time.effective_speed();
    let source = virtual_time.delta_secs() / virtual_time.relative_speed();
    println!(
        "raw={:?} world={:?} effective={:?} configured={:?} previous={previous:?} source={source:?}",
        real.delta_secs(), virtual_time.delta_secs(), virtual_time.effective_speed(),
        virtual_time.relative_speed()
    );
    assert!((previous - 0.1).abs() < 1e-7);
    assert!((source - 0.05).abs() < 1e-7);
}
