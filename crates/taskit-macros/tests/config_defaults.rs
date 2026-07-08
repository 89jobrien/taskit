use taskit_macros::ConfigDefaults;

#[derive(ConfigDefaults)]
struct TestConfig {
    #[default_value = "main"]
    branch: Option<String>,

    #[default_value = "80.0"]
    threshold: Option<f64>,

    // Field without default_value should not generate a method
    #[allow(dead_code)]
    other: Option<String>,
}

#[test]
fn string_none_returns_default() {
    let cfg = TestConfig {
        branch: None,
        threshold: None,
        other: None,
    };
    assert_eq!(cfg.branch(), "main");
}

#[test]
fn string_some_returns_value() {
    let cfg = TestConfig {
        branch: Some("develop".into()),
        threshold: None,
        other: None,
    };
    assert_eq!(cfg.branch(), "develop");
}

#[test]
fn f64_none_returns_default() {
    let cfg = TestConfig {
        branch: None,
        threshold: None,
        other: None,
    };
    assert!((cfg.threshold() - 80.0).abs() < f64::EPSILON);
}

#[test]
fn f64_some_returns_value() {
    let cfg = TestConfig {
        branch: None,
        threshold: Some(95.0),
        other: None,
    };
    assert!((cfg.threshold() - 95.0).abs() < f64::EPSILON);
}
