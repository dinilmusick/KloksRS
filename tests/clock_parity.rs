use kloksrs::*;

fn new_clock() -> ClockService {
    ClockService::new(ClockOptions::default()).unwrap()
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

#[test]
fn starts_pauses_resumes_and_stops_without_resetting_position() {
    let mut clock = new_clock();
    assert_eq!(clock.snapshot(0.0).unwrap().bar, 1);
    clock.start(100.0).unwrap();
    clock.start(150.0).unwrap(); // idempotent: original anchor retained
    let p = clock.snapshot(600.0).unwrap();
    near(p.beat, 1.0);
    assert_eq!(p.tick, 24);
    assert_eq!(p.bar, 1);
    assert_eq!(clock.pause(600.0).unwrap().len(), 24);
    near(clock.snapshot(1600.0).unwrap().beat, 1.0);
    clock.resume(1600.0).unwrap();
    let p = clock.snapshot(2100.0).unwrap();
    near(p.beat, 2.0);
    assert_eq!(p.tick, 48);
    clock.stop(2100.0).unwrap();
    near(clock.snapshot(5000.0).unwrap().beat, 2.0);
    clock.start(5000.0).unwrap();
    near(clock.snapshot(5500.0).unwrap().beat, 3.0);
    clock.reset().unwrap();
    near(clock.snapshot(10000.0).unwrap().beat, 0.0);
}

#[test]
fn tempo_changes_are_continuous_and_signature_changes_update_bar() {
    let mut clock = new_clock();
    clock.start(0.0).unwrap();
    clock.set_tempo(60.0, 500.0).unwrap();
    near(clock.snapshot(1500.0).unwrap().beat, 2.0);
    clock.set_time_signature(6, 8).unwrap();
    let p = clock.snapshot(2250.0).unwrap();
    near(p.beat, 2.75);
    near(p.beat_in_bar, 3.75);
    assert_eq!(p.bar, 1);
    let p = clock.snapshot(2500.0).unwrap();
    assert_eq!(p.bar, 2);
    near(p.beat_in_bar, 1.0);
}

#[test]
fn missed_ticks_are_bounded_and_skip_policy_reports_dropped_events() {
    let mut catchup = ClockService::new(ClockOptions {
        resolution: 4,
        max_catch_up_ticks: 3,
        ..ClockOptions::default()
    })
    .unwrap();
    catchup.start(0.0).unwrap();
    let events = catchup.poll(1000.0).unwrap(); // eight ticks due, last three delivered
    assert_eq!(events.len(), 3);
    assert_eq!(
        events.iter().map(|e| e.position.tick).collect::<Vec<_>>(),
        vec![6, 7, 8]
    );
    assert_eq!(events[2].missed_ticks, 5);
    assert_eq!(catchup.poll(1000.0).unwrap().len(), 0);

    let mut skip = ClockService::new(ClockOptions {
        resolution: 4,
        missed_tick_policy: MissedTickPolicy::Skip,
        ..ClockOptions::default()
    })
    .unwrap();
    skip.start(0.0).unwrap();
    let events = skip.poll(1000.0).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].position.tick, 8);
    assert_eq!(events[0].missed_ticks, 7);
    near(events[0].scheduled_beat, 2.0);
}

#[test]
fn invalid_parameters_fail_closed_and_dispose_blocks_operations() {
    assert_eq!(
        ClockService::new(ClockOptions {
            tempo: f64::NAN,
            ..ClockOptions::default()
        })
        .unwrap_err(),
        ClockError::InvalidTempo
    );
    assert_eq!(
        ClockService::new(ClockOptions {
            resolution: 0,
            ..ClockOptions::default()
        })
        .unwrap_err(),
        ClockError::InvalidResolution
    );
    assert_eq!(
        ClockService::new(ClockOptions {
            denominator: 3,
            ..ClockOptions::default()
        })
        .unwrap_err(),
        ClockError::InvalidSignature
    );
    assert_eq!(
        ClockService::new(ClockOptions {
            max_catch_up_ticks: 0,
            ..ClockOptions::default()
        })
        .unwrap_err(),
        ClockError::InvalidCatchUpLimit
    );
    let mut clock = new_clock();
    assert_eq!(clock.start(-1.0), Err(ClockError::InvalidTime));
    assert_eq!(clock.set_tempo(0.0, 0.0), Err(ClockError::InvalidTempo));
    assert_eq!(
        clock.set_resolution(961, 0.0),
        Err(ClockError::InvalidResolution)
    );
    assert_eq!(
        clock.set_time_signature(33, 8),
        Err(ClockError::InvalidSignature)
    );
    clock.dispose();
    assert_eq!(clock.start(0.0), Err(ClockError::Disposed));
    assert_eq!(clock.reset(), Err(ClockError::Disposed));
}

#[test]
fn tick_subdivision_and_elapsed_time_track_musical_position() {
    let mut clock = ClockService::new(ClockOptions {
        tempo: 120.0,
        resolution: 4,
        numerator: 3,
        denominator: 4,
        ..ClockOptions::default()
    })
    .unwrap();
    clock.start(50.0).unwrap();
    let p = clock.snapshot(2050.0).unwrap();
    assert_eq!(p.bar, 2);
    near(p.beat_in_bar, 2.0);
    assert_eq!(p.tick, 16);
    assert_eq!(p.subdivision, 0);
    near(p.elapsed_ms, 2000.0);
    assert_eq!(clock.poll(2050.0).unwrap().len(), 16);
}
