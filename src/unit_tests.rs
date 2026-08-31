extern crate std;

use super::*;

#[test]
fn tables_provenance() {
    assert_eq!(UNICODE_VERSION, (17, 0, 0));
    assert_eq!(unicode_tables::TABLE_BYTES, 31_658);
    assert!(unicode_tables::TABLE_BYTES <= 40 * 1024);
    assert_eq!(
        unicode_tables::INPUT_HASHES,
        [
            "e6a18fa91f8f6a6f8e534b1d3f128c21ada45bfe152eb6b1bcc5e15fd8ac92e6",
            "ea7ce50f3444a050333448dffef1cadd9325af55cbb764b4a2280faf52170a33",
            "d62e5bab70ca74f099343f71224fa051cb1fdd61a1ab45c0488c44cfc0b6102e",
            "2cb2bb9455cda83e8481541ecf5b6dfda66a3bb89efa3fa7c5297eccf607b72b",
        ]
    );
}

#[test]
fn explicit_defaults_override_header_prose() {
    assert_eq!(break_property(0x1f8ff), BreakClass::Unknown);
    assert_eq!(break_property(0x1fffd), BreakClass::Ideographic);
    assert_eq!(break_property(0x11_0000), BreakClass::Unknown);
    assert_eq!(break_property(u32::MAX), BreakClass::Unknown);
}

#[test]
fn empty_and_hard_breaks() {
    assert_eq!(linebreaks("").next(), None);
    assert_eq!(
        linebreaks("a\r\nb\nc\u{000b}d").collect::<std::vec::Vec<_>>(),
        [
            (3, BreakOpportunity::Mandatory),
            (5, BreakOpportunity::Mandatory),
            (7, BreakOpportunity::Mandatory),
            (8, BreakOpportunity::Mandatory),
        ]
    );
}
