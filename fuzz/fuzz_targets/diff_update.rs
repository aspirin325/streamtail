#![no_main]

use libfuzzer_sys::fuzz_target;
use streamtail::diff::DiffEngine;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let split_at = input
        .char_indices()
        .nth(data.len() % input.chars().count().saturating_add(1))
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    let first = &input[..split_at];
    let second = &input[split_at..];

    let mut engine = DiffEngine::new();
    let _initial = engine.update(first);
    let _next = engine.update(second);
});
