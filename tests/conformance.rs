use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeMap;

use uax14_linebreak::{
    break_property, linebreaks, linebreaks_iter, split_at_safe, BreakClass, BreakOpportunity,
};

struct CountingAllocator;

std::thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        System.realloc(pointer, layout, size)
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn count_allocation() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

const LINE_BREAK_CASES: &str = include_str!("data/line-break-test-17.txt");
const VECTORS: &str = include_str!("data/conformance-vectors.json");

#[test]
fn conformance_linebreaktest() {
    let mut count = 0;
    for (line_number, line) in LINE_BREAK_CASES.lines().enumerate() {
        let (text, expected) = parse_line_break_case(line);
        let actual: Vec<_> = linebreaks(&text).collect();
        assert_eq!(actual, expected, "case {}: {line}", line_number + 1);
        count += 1;
    }
    assert_eq!(count, 19_338);
}

#[test]
fn conformance_iterator_allocates_nothing() {
    for line in LINE_BREAK_CASES.lines() {
        let (text, _) = parse_line_break_case(line);
        ALLOCATIONS.with(|count| count.set(0));
        COUNTING.with(|counting| counting.set(true));
        let mut count = 0;
        for _ in linebreaks(&text) {
            count += 1;
        }
        COUNTING.with(|counting| counting.set(false));
        let allocations = ALLOCATIONS.with(Cell::get);
        assert_eq!(allocations, 0, "{count} opportunities for {line}");
    }
}

#[test]
fn skew_vectors() {
    let root = parse_json(VECTORS);
    for vector in array_field(&root, "skew") {
        let id = string_field(vector, "id");
        let input = string_field(vector, "input");
        let expected = break_list(object_field(vector, "expected"));
        assert_eq!(linebreaks(input).collect::<Vec<_>>(), expected, "{id}");
    }
}

#[test]
fn split_regression_issue6() {
    let root = parse_json(VECTORS);
    for vector in array_field(&root, "split_at_safe") {
        let id = string_field(vector, "id");
        let input = string_field(vector, "input");
        let expected = number_field(vector, "expected_prev_len");
        let (before, after) = split_at_safe(input);
        assert_eq!(before.len(), expected, "{id}");
        assert_eq!(before.len() + after.len(), input.len(), "{id}");
        assert_eq!(format!("{before}{after}"), input, "{id}");
    }
}

#[test]
fn split_prefix_battery() {
    let prefixes = [
        "[ ",
        "“ ",
        "— ",
        "\u{200b} ",
        "א",
        "!\u{301}",
        "🇫🇷🇩",
        "%",
        "$",
    ];
    let root = parse_json(VECTORS);
    for vector in array_field(&root, "split_at_safe") {
        let id = string_field(vector, "id");
        let input = string_field(vector, "input");
        let split = number_field(vector, "expected_prev_len");
        if split == 0 {
            continue;
        }
        let suffix = &input[split..];
        let suffix_breaks: Vec<_> = linebreaks(suffix).collect();
        for prefix in prefixes {
            let joined = format!("{prefix}{input}");
            let base = prefix.len() + split;
            let shifted: Vec<_> = linebreaks(&joined)
                .filter(|(index, _)| *index > base)
                .map(|(index, kind)| (index - base, kind))
                .collect();
            assert_eq!(shifted, suffix_breaks, "{id} with prefix {prefix:?}");
        }
    }
}

#[test]
fn split_sa_combining_restart() {
    let input = "-\u{1033}[";
    let (before, suffix) = split_at_safe(input);
    assert!(before.is_empty());
    assert_eq!(suffix, input);
}

#[test]
fn iter_api() {
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Marker(&'static str);

    let input = [
        (Marker("alpha"), 'a'),
        (Marker("space"), ' '),
        (Marker("beta"), 'b'),
    ];
    let breaks: Vec<_> = linebreaks_iter(input.iter().cloned(), Marker("end")).collect();
    assert_eq!(
        breaks,
        [
            (Marker("beta"), BreakOpportunity::Allowed),
            (Marker("end"), BreakOpportunity::Mandatory),
        ]
    );

    for line in LINE_BREAK_CASES.lines() {
        let (text, _) = parse_line_break_case(line);
        assert_eq!(
            linebreaks(&text).collect::<Vec<_>>(),
            linebreaks_iter(text.char_indices(), text.len()).collect::<Vec<_>>()
        );
    }
}

fn parse_line_break_case(line: &str) -> (String, Vec<(usize, BreakOpportunity)>) {
    let mut tokens = line.split_whitespace();
    let mut marker = tokens.next().expect("leading marker");
    let mut text = String::new();
    let mut expected = Vec::new();
    let mut previous_class = None;
    while let Some(point) = tokens.next() {
        if marker == "÷" && !text.is_empty() {
            let kind = if matches!(
                previous_class,
                Some(
                    BreakClass::MandatoryBreak
                        | BreakClass::CarriageReturn
                        | BreakClass::LineFeed
                        | BreakClass::NextLine
                )
            ) {
                BreakOpportunity::Mandatory
            } else {
                BreakOpportunity::Allowed
            };
            expected.push((text.len(), kind));
        }
        let point = u32::from_str_radix(point, 16).expect("hex code point");
        text.push(char::from_u32(point).expect("Unicode scalar value"));
        previous_class = Some(break_property(point));
        marker = tokens.next().expect("trailing marker");
    }
    assert_eq!(marker, "÷");
    if let Some(last) = expected.last_mut() {
        if last.0 == text.len() {
            last.1 = BreakOpportunity::Mandatory;
        } else {
            expected.push((text.len(), BreakOpportunity::Mandatory));
        }
    } else if !text.is_empty() {
        expected.push((text.len(), BreakOpportunity::Mandatory));
    }
    (text, expected)
}

#[derive(Debug)]
enum Json {
    Null,
    Bool,
    Number(usize),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

fn parse_json(text: &str) -> Json {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        position: 0,
    };
    let value = parser.value();
    parser.space();
    assert_eq!(parser.position, parser.bytes.len());
    value
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Parser<'_> {
    fn value(&mut self) -> Json {
        self.space();
        match self.peek() {
            b'n' => {
                self.literal(b"null");
                Json::Null
            }
            b't' => {
                self.literal(b"true");
                Json::Bool
            }
            b'f' => {
                self.literal(b"false");
                Json::Bool
            }
            b'"' => Json::String(self.string()),
            b'[' => self.array(),
            b'{' => self.object(),
            b'0'..=b'9' => self.number(),
            byte => panic!("unexpected JSON byte {byte} at {}", self.position),
        }
    }

    fn array(&mut self) -> Json {
        self.take(b'[');
        let mut values = Vec::new();
        self.space();
        if self.peek() != b']' {
            loop {
                values.push(self.value());
                self.space();
                if self.peek() == b']' {
                    break;
                }
                self.take(b',');
            }
        }
        self.take(b']');
        Json::Array(values)
    }

    fn object(&mut self) -> Json {
        self.take(b'{');
        let mut values = BTreeMap::new();
        self.space();
        if self.peek() != b'}' {
            loop {
                self.space();
                let key = self.string();
                self.space();
                self.take(b':');
                values.insert(key, self.value());
                self.space();
                if self.peek() == b'}' {
                    break;
                }
                self.take(b',');
            }
        }
        self.take(b'}');
        Json::Object(values)
    }

    fn string(&mut self) -> String {
        self.take(b'"');
        let mut value = String::new();
        loop {
            let byte = self.next();
            match byte {
                b'"' => return value,
                b'\\' => match self.next() {
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'/' => value.push('/'),
                    b'b' => value.push('\u{0008}'),
                    b'f' => value.push('\u{000c}'),
                    b'n' => value.push('\n'),
                    b'r' => value.push('\r'),
                    b't' => value.push('\t'),
                    b'u' => {
                        let hex =
                            std::str::from_utf8(&self.bytes[self.position..self.position + 4])
                                .unwrap();
                        self.position += 4;
                        value.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap());
                    }
                    escape => panic!("unknown escape {escape}"),
                },
                0x00..=0x7f => value.push(char::from(byte)),
                _ => {
                    self.position -= 1;
                    let tail = std::str::from_utf8(&self.bytes[self.position..]).unwrap();
                    let character = tail.chars().next().unwrap();
                    self.position += character.len_utf8();
                    value.push(character);
                }
            }
        }
    }

    fn number(&mut self) -> Json {
        let start = self.position;
        while self.peek().is_ascii_digit() {
            self.position += 1;
        }
        let value = std::str::from_utf8(&self.bytes[start..self.position]).unwrap();
        Json::Number(value.parse().unwrap())
    }

    fn literal(&mut self, literal: &[u8]) {
        assert_eq!(
            &self.bytes[self.position..self.position + literal.len()],
            literal
        );
        self.position += literal.len();
    }

    fn space(&mut self) {
        while self.position < self.bytes.len() && self.bytes[self.position].is_ascii_whitespace() {
            self.position += 1;
        }
    }

    fn take(&mut self, expected: u8) {
        self.space();
        assert_eq!(self.next(), expected);
    }

    fn peek(&self) -> u8 {
        self.bytes.get(self.position).copied().unwrap_or(0)
    }

    fn next(&mut self) -> u8 {
        let byte = self.peek();
        self.position += 1;
        byte
    }
}

fn object_field<'a>(object: &'a Json, field: &str) -> &'a Json {
    match object {
        Json::Object(fields) => fields
            .get(field)
            .unwrap_or_else(|| panic!("missing {field}")),
        _ => panic!("expected object"),
    }
}

fn array_field<'a>(object: &'a Json, field: &str) -> &'a [Json] {
    match object_field(object, field) {
        Json::Array(values) => values,
        _ => panic!("expected array {field}"),
    }
}

fn string_field<'a>(object: &'a Json, field: &str) -> &'a str {
    match object_field(object, field) {
        Json::String(value) => value,
        _ => panic!("expected string {field}"),
    }
}

fn number_field(object: &Json, field: &str) -> usize {
    match object_field(object, field) {
        Json::Number(value) => *value,
        _ => panic!("expected number {field}"),
    }
}

fn break_list(value: &Json) -> Vec<(usize, BreakOpportunity)> {
    match value {
        Json::Array(items) => items
            .iter()
            .map(|item| match item {
                Json::Array(pair) => {
                    let index = match &pair[0] {
                        Json::Number(value) => *value,
                        _ => panic!("break index"),
                    };
                    let kind = match &pair[1] {
                        Json::String(value) if value == "Allowed" => BreakOpportunity::Allowed,
                        Json::String(value) if value == "Mandatory" => BreakOpportunity::Mandatory,
                        _ => panic!("break kind"),
                    };
                    (index, kind)
                }
                _ => panic!("break pair"),
            })
            .collect(),
        _ => panic!("break list"),
    }
}
