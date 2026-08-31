use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CODEPOINTS: usize = 0x11_0000;
const PAGE_LEN: usize = 64;
const GROUP_LEN: usize = 16;

/// Line_Break short names in table order. The index of a name here is the
/// class discriminant in src/lib.rs.
const CLASS_NAMES: [&str; 49] = [
    "AI", "AK", "AL", "AP", "AS", "B2", "BA", "BB", "BK", "CB", "CJ", "CL", "CM", "CP", "CR", "EB",
    "EM", "EX", "GL", "H2", "H3", "HH", "HL", "HY", "ID", "IN", "IS", "JL", "JT", "JV", "LF", "NL",
    "NS", "NU", "OP", "PO", "PR", "QU", "RI", "SA", "SG", "SP", "SY", "VF", "VI", "WJ", "XX", "ZW",
    "ZWJ",
];

// Flag bits, matching the Property constants in src/lib.rs.
const EAST_ASIAN: u8 = 1;
const PI: u8 = 2;
const PF: u8 = 4;
const EXTENDED_PICTOGRAPHIC_CN: u8 = 8;
const DOTTED_CIRCLE: u8 = 16;
const COMBINING_CATEGORY: u8 = 32;

fn class(name: &str) -> u8 {
    CLASS_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .unwrap_or_else(|| panic!("unknown class name {name}")) as u8
}

fn ranges(path: &Path) -> Vec<(usize, usize, String)> {
    let text = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", path.display());
    });
    text.lines()
        .filter_map(|line| {
            let body = line.split('#').next().unwrap().trim();
            if body.is_empty() || body.starts_with('@') {
                return None;
            }
            let (span, value) = body.split_once(';')?;
            let span = span.trim();
            let value = value.split_whitespace().next()?.to_owned();
            let (start, end) = match span.split_once("..") {
                Some((start, end)) => (hex(start), hex(end)),
                None => {
                    let point = hex(span);
                    (point, point)
                }
            };
            Some((start, end, value))
        })
        .collect()
}

fn hex(value: &str) -> usize {
    usize::from_str_radix(value.trim(), 16).unwrap()
}

fn fill(path: &Path, values: &mut [u8], names: &[&str]) {
    for (start, end, value) in ranges(path) {
        let index = names
            .iter()
            .position(|candidate| *candidate == value)
            .unwrap_or_else(|| panic!("unknown value {value} in {}", path.display()));
        values[start..=end].fill(index as u8);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    let bit_length = (bytes.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_length.to_be_bytes());
    for block in data.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for (index, word) in block.chunks_exact(4).enumerate() {
            w[index] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut text = String::new();
    for value in state {
        write!(text, "{value:08x}").unwrap();
    }
    text
}

fn main() {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../.."));
    let data = root.join("ucd-17.0.0");
    let output = root.join("crate/src/unicode_tables.rs");

    let input_files = [
        "LineBreak.txt",
        "EastAsianWidth.txt",
        "DerivedGeneralCategory.txt",
        "emoji-data.txt",
    ];
    let input_hashes: Vec<String> = input_files
        .iter()
        .map(|name| {
            let path = data.join(name);
            let bytes = fs::read(&path).unwrap_or_else(|error| {
                panic!("failed to read {}: {error}", path.display());
            });
            sha256_hex(&bytes)
        })
        .collect();

    let mut line_break = vec![class("XX"); CODEPOINTS];
    fill(&data.join("LineBreak.txt"), &mut line_break, &CLASS_NAMES);

    let mut combining = vec![false; CODEPOINTS];
    let mut pi = vec![false; CODEPOINTS];
    let mut pf = vec![false; CODEPOINTS];
    let mut cn = vec![false; CODEPOINTS];
    for (start, end, value) in ranges(&data.join("DerivedGeneralCategory.txt")) {
        let target = match value.as_str() {
            "Mn" | "Mc" => Some(&mut combining),
            "Pi" => Some(&mut pi),
            "Pf" => Some(&mut pf),
            "Cn" => Some(&mut cn),
            _ => None,
        };
        if let Some(target) = target {
            target[start..=end].fill(true);
        }
    }

    let mut east_asian = vec![false; CODEPOINTS];
    for (start, end, value) in ranges(&data.join("EastAsianWidth.txt")) {
        if matches!(value.as_str(), "F" | "W" | "H") {
            east_asian[start..=end].fill(true);
        }
    }

    let mut extended_pictographic = vec![false; CODEPOINTS];
    for (start, end, value) in ranges(&data.join("emoji-data.txt")) {
        if value == "Extended_Pictographic" {
            extended_pictographic[start..=end].fill(true);
        }
    }

    let mut properties = Vec::<(u8, u8)>::new();
    let mut property_ids = BTreeMap::<(u8, u8), u8>::new();
    let mut cells = vec![0_u8; CODEPOINTS];
    for point in 0..CODEPOINTS {
        let mut flags = 0_u8;
        if east_asian[point] {
            flags |= EAST_ASIAN;
        }
        if line_break[point] == class("QU") && pi[point] {
            flags |= PI;
        }
        if line_break[point] == class("QU") && pf[point] {
            flags |= PF;
        }
        if extended_pictographic[point] && cn[point] {
            flags |= EXTENDED_PICTOGRAPHIC_CN;
        }
        if point == 0x25cc {
            flags |= DOTTED_CIRCLE;
        }
        if line_break[point] == class("SA") && combining[point] {
            flags |= COMBINING_CATEGORY;
        }
        let key = (line_break[point], flags);
        let id = match property_ids.get(&key) {
            Some(id) => *id,
            None => {
                let id = u8::try_from(properties.len()).expect("more than 256 properties");
                properties.push(key);
                property_ids.insert(key, id);
                id
            }
        };
        cells[point] = id;
    }

    let mut pages = Vec::<Vec<u8>>::new();
    let mut page_ids = BTreeMap::<Vec<u8>, u16>::new();
    let mut page_map = Vec::<u16>::new();
    for page in cells.chunks_exact(PAGE_LEN) {
        let page = page.to_vec();
        let id = match page_ids.get(&page) {
            Some(id) => *id,
            None => {
                let id = u16::try_from(pages.len()).expect("more than 65536 unique pages");
                pages.push(page.clone());
                page_ids.insert(page, id);
                id
            }
        };
        page_map.push(id);
    }

    let mut groups = Vec::<Vec<u16>>::new();
    let mut group_ids = BTreeMap::<Vec<u16>, u16>::new();
    let mut group_map = Vec::<u16>::new();
    for group in page_map.chunks_exact(GROUP_LEN) {
        let group = group.to_vec();
        let id = match group_ids.get(&group) {
            Some(id) => *id,
            None => {
                let id = u16::try_from(groups.len()).expect("more than 65536 unique groups");
                groups.push(group.clone());
                group_ids.insert(group, id);
                id
            }
        };
        group_map.push(id);
    }

    let (safe_restart_classes, unsafe_restart_flags) = derive_restart(&properties);

    let table_bytes = group_map.len() * 2
        + groups.len() * GROUP_LEN * 2
        + pages.len() * PAGE_LEN
        + 256
        + properties.len() * 2;
    let mut text = String::new();
    writeln!(text, "// Unicode 17.0.0 table input hashes:").unwrap();
    for (name, hash) in input_files.iter().zip(&input_hashes) {
        writeln!(text, "// {name} {hash}").unwrap();
    }
    writeln!(text, "#[cfg(test)]").unwrap();
    writeln!(text, "pub(crate) const TABLE_BYTES: usize = {table_bytes};").unwrap();
    writeln!(
        text,
        "pub(crate) const SAFE_RESTART_CLASSES: u64 = {safe_restart_classes:#018x};",
    )
    .unwrap();
    writeln!(
        text,
        "pub(crate) const UNSAFE_RESTART_FLAGS: u8 = {unsafe_restart_flags:#04x};"
    )
    .unwrap();
    writeln!(text, "#[cfg(test)]").unwrap();
    writeln!(text, "pub(crate) const INPUT_HASHES: [&str; 4] = [").unwrap();
    for hash in &input_hashes {
        writeln!(text, "    \"{hash}\",").unwrap();
    }
    writeln!(text, "];\n").unwrap();
    let packed: Vec<u16> = properties
        .iter()
        .map(|property| u16::from(property.0) | (u16::from(property.1) << 8))
        .collect();
    let ascii: Vec<u16> = cells[..128]
        .iter()
        .map(|id| packed[usize::from(*id)])
        .collect();
    write_u16_array(&mut text, "ASCII_PROPERTY", &ascii);
    write_u16_array(&mut text, "GROUP_MAP", &group_map);
    writeln!(
        text,
        "pub(crate) static GROUPS: [[u16; {GROUP_LEN}]; {}] = [",
        groups.len()
    )
    .unwrap();
    for group in &groups {
        write!(text, "    [").unwrap();
        for (index, value) in group.iter().enumerate() {
            if index != 0 {
                text.push_str(", ");
            }
            write!(text, "{value}").unwrap();
        }
        text.push_str("],\n");
    }
    text.push_str("];\n");
    writeln!(
        text,
        "pub(crate) static PAGES: [[u8; {PAGE_LEN}]; {}] = [",
        pages.len()
    )
    .unwrap();
    for page in &pages {
        write!(text, "    [").unwrap();
        for (index, value) in page.iter().enumerate() {
            if index != 0 {
                text.push_str(", ");
            }
            write!(text, "{value}").unwrap();
        }
        text.push_str("],\n");
    }
    text.push_str("];\n");
    write_u16_array(&mut text, "PROPERTIES", &packed);
    fs::write(&output, text).unwrap_or_else(|error| {
        panic!("failed to write {}: {error}", output.display());
    });
    let rustfmt = env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into());
    let status = Command::new(rustfmt)
        .arg(&output)
        .status()
        .expect("failed to run rustfmt");
    assert!(status.success(), "rustfmt failed");
    eprintln!(
        "wrote {} properties, {} pages, {} groups, and {} table bytes",
        properties.len(),
        pages.len(),
        groups.len(),
        table_bytes
    );
}

// Restart derivation. A class is a safe restart when the rule machine in
// src/lib.rs reaches the same decision-relevant state after consuming one
// character of that class from every possible predecessor as it does from
// the start of text. The functions below mirror the machine's transitions
// (`normalize`, `State::new`, `State::push`) and its reads of
// `state.previous` in `decide`, then simulate every predecessor value. The
// restart set falls out of the simulation instead of being written down.

/// LB1 resolution as `resolved_property` in src/lib.rs applies it.
fn resolve(class_id: u8, flags: u8) -> u8 {
    match CLASS_NAMES[class_id as usize] {
        "AI" | "SG" | "XX" => class("AL"),
        "SA" if flags & COMBINING_CATEGORY != 0 => class("CM"),
        "SA" => class("AL"),
        "CJ" => class("NS"),
        _ => class_id,
    }
}

/// True when `normalize` in src/lib.rs can fold the character into a
/// predecessor under LB9, so a restarted analysis would see a first
/// character the full analysis never surfaced.
fn absorbable(resolved_id: u8) -> bool {
    resolved_id == class("CM") || resolved_id == class("ZWJ")
}

/// True when `decide` in src/lib.rs reads `state.previous` while `last`
/// holds this class. LB19a reads the class before a quote, LB20a and LB21a
/// read the class before a hyphen, and LB28a reads the class before a
/// virama.
fn reads_previous(resolved_id: u8) -> bool {
    ["QU", "HY", "HH", "VI"].map(class).contains(&resolved_id)
}

/// Mirror of `pi_prefix` in src/lib.rs.
fn pi_prefix(class_id: u8) -> bool {
    ["BK", "CR", "LF", "NL", "OP", "QU", "GL", "SP", "ZW"]
        .map(class)
        .contains(&class_id)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Numeric {
    Outside,
    Number,
    Closed,
}

/// Mirror of the LB25 numeric transition in `State::push`.
fn next_numeric(old: Numeric, class_id: u8) -> Numeric {
    if class_id == class("NU") {
        return Numeric::Number;
    }
    if old == Numeric::Number {
        if ["SY", "IS"].map(class).contains(&class_id) {
            return Numeric::Number;
        }
        if ["CL", "CP"].map(class).contains(&class_id) {
            return Numeric::Closed;
        }
    }
    Numeric::Outside
}

/// The post-push fields a later decision can read, except `previous`,
/// which `reads_previous` covers.
#[derive(Eq, PartialEq)]
struct PostState {
    anchor_holds_predecessor: bool,
    pi_run: bool,
    numeric: Numeric,
    parity_odd: bool,
}

/// Mirror of `State::push` in src/lib.rs, applied to one character of the
/// given resolved class after the given predecessor values.
fn pushed_state(
    predecessor: u8,
    old_pi_run: bool,
    old_numeric: Numeric,
    old_parity: bool,
    resolved_id: u8,
    flags: u8,
) -> PostState {
    let space = resolved_id == class("SP");
    PostState {
        anchor_holds_predecessor: space,
        pi_run: if space {
            old_pi_run
        } else if resolved_id == class("QU") && flags & PI != 0 {
            pi_prefix(predecessor)
        } else {
            false
        },
        numeric: next_numeric(old_numeric, resolved_id),
        parity_odd: if resolved_id == class("RI") {
            !old_parity
        } else {
            false
        },
    }
}

/// Mirror of `State::new` in src/lib.rs.
fn fresh_state(resolved_id: u8, flags: u8) -> PostState {
    PostState {
        anchor_holds_predecessor: false,
        pi_run: resolved_id == class("QU") && flags & PI != 0,
        numeric: next_numeric(Numeric::Outside, resolved_id),
        parity_odd: resolved_id == class("RI"),
    }
}

/// True when some predecessor leaves a trace the fresh state lacks.
fn required_exclusion(class_id: u8, flags: u8) -> bool {
    let resolved_id = resolve(class_id, flags);
    if absorbable(resolved_id) || reads_previous(resolved_id) {
        return true;
    }
    let fresh = fresh_state(resolved_id, flags);
    for predecessor in 0..CLASS_NAMES.len() as u8 {
        for old_pi_run in [false, true] {
            for old_numeric in [Numeric::Outside, Numeric::Number, Numeric::Closed] {
                for old_parity in [false, true] {
                    let pushed = pushed_state(
                        predecessor,
                        old_pi_run,
                        old_numeric,
                        old_parity,
                        resolved_id,
                        flags,
                    );
                    if pushed != fresh {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Conservative exclusions on top of the simulation. B2, CB and OP anchor
/// the bridged rules LB17, LB20a and LB14. The simulation shows a restart
/// at any of the three reproduces the full analysis, so this margin only
/// moves splits earlier and can never corrupt a suffix. The shipped split
/// vectors record the resulting choice, so widening the set is a breaking
/// data change.
fn pinned_exclusion(class_id: u8) -> bool {
    ["B2", "CB", "OP"].map(class).contains(&class_id)
}

/// Derives the restart bitset and the excluded flag bits from the property
/// variants that occur in the table.
fn derive_restart(properties: &[(u8, u8)]) -> (u64, u8) {
    // A flag bit joins the exclusion mask when a variant carrying it needs
    // exclusion while the same class keeps a variant that does not.
    // DOTTED_CIRCLE joins by contract: U+25CC anchors LB28a alongside the
    // aksara classes and stays out of the restart set with them.
    let mut flag_mask = DOTTED_CIRCLE;
    for bit in [
        EAST_ASIAN,
        PI,
        PF,
        EXTENDED_PICTOGRAPHIC_CN,
        COMBINING_CATEGORY,
    ] {
        for &(class_id, flags) in properties {
            if flags & bit == 0 || !required_exclusion(class_id, flags) {
                continue;
            }
            let class_survives = properties.iter().any(|&(candidate, candidate_flags)| {
                candidate == class_id
                    && candidate_flags & bit == 0
                    && !required_exclusion(candidate, candidate_flags)
            });
            if class_survives {
                flag_mask |= bit;
            }
        }
    }
    let mut bits = 0_u64;
    for class_id in 0..CLASS_NAMES.len() as u8 {
        let excluded = pinned_exclusion(class_id)
            || properties.iter().any(|&(candidate, flags)| {
                candidate == class_id
                    && flags & flag_mask == 0
                    && required_exclusion(candidate, flags)
            });
        if !excluded {
            bits |= 1_u64 << class_id;
        }
    }
    // The split contract names these classes as never being restart
    // points. Fail generation rather than ship a set that readmits one.
    for name in ["CM", "ZWJ", "SP", "RI", "HY", "B2", "QU", "CB"] {
        assert!(
            bits & (1_u64 << class(name)) == 0,
            "{name} must stay out of the restart set"
        );
    }
    (bits, flag_mask)
}

fn write_u16_array(text: &mut String, name: &str, values: &[u16]) {
    writeln!(
        text,
        "pub(crate) static {name}: [u16; {}] = [",
        values.len()
    )
    .unwrap();
    for chunk in values.chunks(32) {
        text.push_str("    ");
        for (index, value) in chunk.iter().enumerate() {
            if index != 0 {
                text.push_str(", ");
            }
            write!(text, "{value}").unwrap();
        }
        text.push_str(",\n");
    }
    text.push_str("];\n");
}
