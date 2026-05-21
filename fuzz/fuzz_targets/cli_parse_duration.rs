#![no_main]

use libfuzzer_sys::fuzz_target;
use streamtail::cli::parse_duration;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let _parsed = parse_duration(&input);
});
