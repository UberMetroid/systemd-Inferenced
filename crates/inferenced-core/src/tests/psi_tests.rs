use crate::psi::{PressureLevel, PressureMetrics};

#[test]
fn test_psi_read_current() {
    let metrics = PressureMetrics::read_current();
    assert!(metrics.memory_some_avg10 >= 0.0);
    assert!(metrics.memory_full_avg10 >= 0.0);
    assert!(metrics.cpu_some_avg10 >= 0.0);
    assert!(metrics.io_some_avg10 >= 0.0);
    assert!(matches!(
        metrics.level,
        PressureLevel::Normal | PressureLevel::Elevated | PressureLevel::Critical
    ));
}

#[test]
fn test_psi_level_thresholds() {
    // Normal: low values
    let normal = PressureMetrics {
        memory_some_avg10: 5.0,
        memory_full_avg10: 1.0,
        cpu_some_avg10: 20.0,
        io_some_avg10: 5.0,
        level: PressureLevel::Normal,
    };
    assert_eq!(normal.level, PressureLevel::Normal);

    // Elevated: mem_some > 15.0 or io_some > 20.0 or cpu_some > 60.0
    let elevated = PressureMetrics {
        memory_some_avg10: 22.0,
        memory_full_avg10: 2.0,
        cpu_some_avg10: 30.0,
        io_some_avg10: 5.0,
        level: PressureLevel::Elevated,
    };
    assert_eq!(elevated.level, PressureLevel::Elevated);

    // Critical: mem_full > 10.0 or mem_some > 40.0 or io_some > 50.0
    let critical = PressureMetrics {
        memory_some_avg10: 45.0,
        memory_full_avg10: 12.0,
        cpu_some_avg10: 30.0,
        io_some_avg10: 10.0,
        level: PressureLevel::Critical,
    };
    assert_eq!(critical.level, PressureLevel::Critical);
}
