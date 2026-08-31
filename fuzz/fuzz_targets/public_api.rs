#![no_main]

use libfuzzer_sys::fuzz_target;
use uax14_linebreak::{break_property, linebreaks, split_at_safe};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let full: Vec<_> = linebreaks(&text).collect();
    let (before, suffix) = split_at_safe(&text);
    let base = before.len();
    if base > 0 {
        let shifted: Vec<_> = full
            .iter()
            .copied()
            .filter(|(index, _)| *index > base)
            .map(|(index, kind)| (index - base, kind))
            .collect();
        assert_eq!(shifted, linebreaks(suffix).collect::<Vec<_>>());
    }
    for chunk in data.chunks(4) {
        let mut bytes = [0_u8; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        let _ = break_property(u32::from_le_bytes(bytes));
    }
});
