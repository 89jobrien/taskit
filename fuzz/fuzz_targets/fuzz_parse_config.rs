#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // parse_config must never panic on arbitrary TOML-like input
        let _: Result<taskit_types::config::Config, _> = toml::from_str(s);
    }
});
