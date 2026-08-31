#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Unicode 17.0.0 line break opportunities.

use core::iter::FusedIterator;
use core::str::CharIndices;

mod unicode_tables;

/// Unicode version of the property tables.
pub const UNICODE_VERSION: (u8, u8, u8) = (17, 0, 0);

/// A Unicode `Line_Break` property value.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
#[repr(u8)]
pub enum BreakClass {
    /// Ambiguous.
    Ambiguous,
    /// Aksara.
    Aksara,
    /// Alphabetic.
    Alphabetic,
    /// Aksara prebase.
    AksaraPrebase,
    /// Aksara start.
    AksaraStart,
    /// Break on either side.
    BreakBoth,
    /// Break after.
    BreakAfter,
    /// Break before.
    BreakBefore,
    /// Mandatory break.
    MandatoryBreak,
    /// Contingent break.
    ContingentBreak,
    /// Conditional Japanese starter.
    ConditionalJapaneseStarter,
    /// Closing punctuation.
    ClosePunctuation,
    /// Combining mark.
    CombiningMark,
    /// Closing parenthesis.
    CloseParenthesis,
    /// Carriage return.
    CarriageReturn,
    /// Emoji base.
    EBase,
    /// Emoji modifier.
    EModifier,
    /// Exclamation or interrogation mark.
    Exclamation,
    /// Nonbreaking glue.
    Glue,
    /// Hangul LV syllable.
    H2,
    /// Hangul LVT syllable.
    H3,
    /// Unambiguous hyphen.
    UnambiguousHyphen,
    /// Hebrew letter.
    HebrewLetter,
    /// Hyphen.
    Hyphen,
    /// Ideographic.
    Ideographic,
    /// Inseparable.
    Inseparable,
    /// Infix numeric separator.
    InfixNumeric,
    /// Hangul leading jamo.
    JL,
    /// Hangul trailing jamo.
    JT,
    /// Hangul vowel jamo.
    JV,
    /// Line feed.
    LineFeed,
    /// Next line.
    NextLine,
    /// Nonstarter.
    Nonstarter,
    /// Numeric.
    Numeric,
    /// Opening punctuation.
    OpenPunctuation,
    /// Postfix numeric.
    PostfixNumeric,
    /// Prefix numeric.
    PrefixNumeric,
    /// Quotation mark.
    Quotation,
    /// Regional indicator.
    RegionalIndicator,
    /// Complex-context character.
    ComplexContext,
    /// Surrogate.
    Surrogate,
    /// Space.
    Space,
    /// Break symbol.
    BreakSymbols,
    /// Final virama.
    ViramaFinal,
    /// Virama.
    Virama,
    /// Word joiner.
    WordJoiner,
    /// Unknown.
    Unknown,
    /// Zero-width space.
    ZWSpace,
    /// Zero-width joiner.
    ZWJ,
}

/// A line break opportunity.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum BreakOpportunity {
    /// A required line break.
    Mandatory,
    /// A permitted line break.
    Allowed,
}

/// Returns the raw `Line_Break` property of a code point.
///
/// Values above `U+10FFFF` return [`BreakClass::Unknown`].
pub fn break_property(codepoint: u32) -> BreakClass {
    property(codepoint).class.public()
}

/// An iterator over line break opportunities in a string.
#[derive(Clone, Debug)]
pub struct Linebreaks<'a> {
    inner: LinebreaksIter<usize, CharIndices<'a>>,
}

impl Iterator for Linebreaks<'_> {
    type Item = (usize, BreakOpportunity);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl FusedIterator for Linebreaks<'_> {}

/// Returns the line break opportunities in `text`.
pub fn linebreaks(text: &str) -> Linebreaks<'_> {
    Linebreaks {
        inner: linebreaks_iter(text.char_indices(), text.len()),
    }
}

/// An iterator over line break opportunities in indexed characters.
///
/// The caller supplies the value reported for the end of the input. Indices
/// pass through unchanged and do not need to be numeric.
#[derive(Clone, Debug)]
pub struct LinebreaksIter<I, It>
where
    It: Iterator<Item = (I, char)> + Clone,
    I: Clone,
{
    chars: It,
    eot: I,
    state: Option<State>,
    finished: bool,
}

/// Returns line break opportunities from indexed characters.
pub fn linebreaks_iter<I, It>(chars: It, eot: I) -> LinebreaksIter<I, It>
where
    It: Iterator<Item = (I, char)> + Clone,
    I: Clone,
{
    LinebreaksIter {
        chars,
        eot,
        state: None,
        finished: false,
    }
}

impl<I, It> Iterator for LinebreaksIter<I, It>
where
    It: Iterator<Item = (I, char)> + Clone,
    I: Clone,
{
    type Item = (I, BreakOpportunity);

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        if self.state.is_none() {
            let (_, first) = match self.chars.next() {
                Some(first) => first,
                None => {
                    self.finished = true;
                    return None;
                }
            };
            let raw = resolved_property(first as u32);
            let significant = normalize(raw, None).expect("initial property is significant");
            self.state = Some(State::new(significant, raw.class == Class::Zwj));
        }

        loop {
            let (index, character) = match self.chars.next() {
                Some(value) => value,
                None => {
                    self.finished = true;
                    return Some((self.eot.clone(), BreakOpportunity::Mandatory));
                }
            };
            let raw = resolved_property(character as u32);
            let state = self.state.as_mut().expect("state was initialized");
            let right = match normalize(raw, Some(state.last)) {
                Some(right) => right,
                None => {
                    state.last_was_zwj = raw.class == Class::Zwj;
                    continue;
                }
            };
            let raw_zwj = raw.class == Class::Zwj;
            let opportunity = match decide_pair(state, right, raw_zwj) {
                Ok(opportunity) => opportunity,
                Err(()) => {
                    let lookahead = if needs_lookahead(state, right) {
                        significant_lookahead(self.chars.clone(), right)
                    } else {
                        [None, None]
                    };
                    let opportunity = decide(state, right, lookahead);
                    state.push(right, raw_zwj);
                    opportunity
                }
            };
            if let Some(opportunity) = opportunity {
                return Some((index, opportunity));
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            (
                0,
                self.chars
                    .size_hint()
                    .1
                    .and_then(|count| count.checked_add(1)),
            )
        }
    }
}

impl<I, It> FusedIterator for LinebreaksIter<I, It>
where
    It: Iterator<Item = (I, char)> + Clone + FusedIterator,
    I: Clone,
{
}

/// Splits a string at its largest safe restart point.
///
/// The second slice can be analyzed without carrying line-breaking state from
/// the first slice. The selected point is always below `text.len()`.
pub fn split_at_safe(text: &str) -> (&str, &str) {
    let split = text
        .char_indices()
        .rev()
        .find(|&(index, character)| {
            index < text.len() && is_safe_restart(property(character as u32))
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    text.split_at(split)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Property {
    class: Class,
    flags: u8,
}

impl Property {
    const EAST_ASIAN: u8 = 1;
    const PI: u8 = 2;
    const PF: u8 = 4;
    const EXTENDED_PICTOGRAPHIC_CN: u8 = 8;
    const DOTTED_CIRCLE: u8 = 16;
    const COMBINING_CATEGORY: u8 = 32;

    fn has(self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    fn is_aksara_or_dotted(self) -> bool {
        matches!(self.class, Class::Ak | Class::As) || self.has(Self::DOTTED_CIRCLE)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
enum Class {
    Ambiguous,
    Ak,
    Al,
    Ap,
    As,
    B2,
    Ba,
    Bb,
    Bk,
    Cb,
    Cj,
    Cl,
    Cm,
    Cp,
    Cr,
    Eb,
    Em,
    Ex,
    Gl,
    H2,
    H3,
    Hh,
    Hl,
    Hy,
    Id,
    In,
    Is,
    Jl,
    Jt,
    Jv,
    Lf,
    Nl,
    Ns,
    Nu,
    Op,
    Po,
    Pr,
    Qu,
    Ri,
    Sa,
    Sg,
    Sp,
    Sy,
    Vf,
    Vi,
    Wj,
    Xx,
    Zw,
    Zwj,
}

impl Class {
    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Ambiguous,
            1 => Self::Ak,
            2 => Self::Al,
            3 => Self::Ap,
            4 => Self::As,
            5 => Self::B2,
            6 => Self::Ba,
            7 => Self::Bb,
            8 => Self::Bk,
            9 => Self::Cb,
            10 => Self::Cj,
            11 => Self::Cl,
            12 => Self::Cm,
            13 => Self::Cp,
            14 => Self::Cr,
            15 => Self::Eb,
            16 => Self::Em,
            17 => Self::Ex,
            18 => Self::Gl,
            19 => Self::H2,
            20 => Self::H3,
            21 => Self::Hh,
            22 => Self::Hl,
            23 => Self::Hy,
            24 => Self::Id,
            25 => Self::In,
            26 => Self::Is,
            27 => Self::Jl,
            28 => Self::Jt,
            29 => Self::Jv,
            30 => Self::Lf,
            31 => Self::Nl,
            32 => Self::Ns,
            33 => Self::Nu,
            34 => Self::Op,
            35 => Self::Po,
            36 => Self::Pr,
            37 => Self::Qu,
            38 => Self::Ri,
            39 => Self::Sa,
            40 => Self::Sg,
            41 => Self::Sp,
            42 => Self::Sy,
            43 => Self::Vf,
            44 => Self::Vi,
            45 => Self::Wj,
            46 => Self::Xx,
            47 => Self::Zw,
            48 => Self::Zwj,
            _ => Self::Xx,
        }
    }

    fn public(self) -> BreakClass {
        match self {
            Self::Ambiguous => BreakClass::Ambiguous,
            Self::Ak => BreakClass::Aksara,
            Self::Al => BreakClass::Alphabetic,
            Self::Ap => BreakClass::AksaraPrebase,
            Self::As => BreakClass::AksaraStart,
            Self::B2 => BreakClass::BreakBoth,
            Self::Ba => BreakClass::BreakAfter,
            Self::Bb => BreakClass::BreakBefore,
            Self::Bk => BreakClass::MandatoryBreak,
            Self::Cb => BreakClass::ContingentBreak,
            Self::Cj => BreakClass::ConditionalJapaneseStarter,
            Self::Cl => BreakClass::ClosePunctuation,
            Self::Cm => BreakClass::CombiningMark,
            Self::Cp => BreakClass::CloseParenthesis,
            Self::Cr => BreakClass::CarriageReturn,
            Self::Eb => BreakClass::EBase,
            Self::Em => BreakClass::EModifier,
            Self::Ex => BreakClass::Exclamation,
            Self::Gl => BreakClass::Glue,
            Self::H2 => BreakClass::H2,
            Self::H3 => BreakClass::H3,
            Self::Hh => BreakClass::UnambiguousHyphen,
            Self::Hl => BreakClass::HebrewLetter,
            Self::Hy => BreakClass::Hyphen,
            Self::Id => BreakClass::Ideographic,
            Self::In => BreakClass::Inseparable,
            Self::Is => BreakClass::InfixNumeric,
            Self::Jl => BreakClass::JL,
            Self::Jt => BreakClass::JT,
            Self::Jv => BreakClass::JV,
            Self::Lf => BreakClass::LineFeed,
            Self::Nl => BreakClass::NextLine,
            Self::Ns => BreakClass::Nonstarter,
            Self::Nu => BreakClass::Numeric,
            Self::Op => BreakClass::OpenPunctuation,
            Self::Po => BreakClass::PostfixNumeric,
            Self::Pr => BreakClass::PrefixNumeric,
            Self::Qu => BreakClass::Quotation,
            Self::Ri => BreakClass::RegionalIndicator,
            Self::Sa => BreakClass::ComplexContext,
            Self::Sg => BreakClass::Surrogate,
            Self::Sp => BreakClass::Space,
            Self::Sy => BreakClass::BreakSymbols,
            Self::Vf => BreakClass::ViramaFinal,
            Self::Vi => BreakClass::Virama,
            Self::Wj => BreakClass::WordJoiner,
            Self::Xx => BreakClass::Unknown,
            Self::Zw => BreakClass::ZWSpace,
            Self::Zwj => BreakClass::ZWJ,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct State {
    last: Property,
    previous: Option<Property>,
    space_anchor: Option<Property>,
    pi_run: bool,
    numeric: NumericState,
    regional_indicator_odd: bool,
    last_was_zwj: bool,
}

impl State {
    #[inline(always)]
    fn new(first: Property, raw_zwj: bool) -> Self {
        Self {
            last: first,
            previous: None,
            space_anchor: None,
            pi_run: first.class == Class::Qu && first.has(Property::PI),
            numeric: if first.class == Class::Nu {
                NumericState::Number
            } else {
                NumericState::None
            },
            regional_indicator_odd: first.class == Class::Ri,
            last_was_zwj: raw_zwj,
        }
    }

    #[inline(always)]
    fn push(&mut self, next: Property, raw_zwj: bool) {
        let old_last = self.last;
        let old_pi_run = self.pi_run;
        if next.class == Class::Sp {
            if old_last.class != Class::Sp {
                self.space_anchor = Some(old_last);
            }
        } else {
            self.space_anchor = None;
        }
        self.pi_run = if next.class == Class::Sp {
            old_pi_run
        } else if next.class == Class::Qu && next.has(Property::PI) {
            pi_prefix(old_last.class)
        } else {
            false
        };
        self.numeric = match (self.numeric, next.class) {
            (_, Class::Nu) => NumericState::Number,
            (NumericState::Number, Class::Sy | Class::Is) => NumericState::Number,
            (NumericState::Number, Class::Cl | Class::Cp) => NumericState::Closed,
            _ => NumericState::None,
        };
        self.regional_indicator_odd = if next.class == Class::Ri {
            !self.regional_indicator_odd
        } else {
            false
        };
        self.previous = Some(old_last);
        self.last = next;
        self.last_was_zwj = raw_zwj;
    }

    #[inline(always)]
    fn push_common(&mut self, next: Property) {
        self.last = next;
        self.previous = None;
        self.space_anchor = None;
        self.pi_run = false;
        self.numeric = NumericState::None;
        self.regional_indicator_odd = false;
        self.last_was_zwj = false;
    }

    #[inline(always)]
    fn ends_with_spaced(&self, class: Class) -> bool {
        self.last.class == class
            || (self.last.class == Class::Sp
                && self.space_anchor.map(|anchor| anchor.class) == Some(class))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum NumericState {
    None,
    Number,
    Closed,
}

#[inline(always)]
fn property(codepoint: u32) -> Property {
    if codepoint < 128 {
        let packed = unicode_tables::ASCII_PROPERTY[codepoint as usize];
        return Property {
            class: Class::from_u8(packed as u8),
            flags: (packed >> 8) as u8,
        };
    }
    if (0x4e00..=0x9fff).contains(&codepoint) {
        return Property {
            class: Class::Id,
            flags: Property::EAST_ASIAN,
        };
    }
    if (0xac00..=0xd7a3).contains(&codepoint) {
        return Property {
            class: if (codepoint - 0xac00) % 28 == 0 {
                Class::H2
            } else {
                Class::H3
            },
            flags: Property::EAST_ASIAN,
        };
    }
    if matches!(
        codepoint,
        0x304a..=0x3062 | 0x3064..=0x3082 | 0x3084 | 0x3086 | 0x3088..=0x308d | 0x308f..=0x3094
    ) {
        return Property {
            class: Class::Id,
            flags: Property::EAST_ASIAN,
        };
    }
    if codepoint == 0x3002 {
        return Property {
            class: Class::Cl,
            flags: Property::EAST_ASIAN,
        };
    }
    if codepoint == 0x0308 {
        return Property {
            class: Class::Cm,
            flags: 0,
        };
    }
    if codepoint > 0x10ffff {
        return Property {
            class: Class::Xx,
            flags: 0,
        };
    }
    let point = codepoint as usize;
    let group = unicode_tables::GROUP_MAP[point >> 10] as usize;
    let page = unicode_tables::GROUPS[group][(point >> 6) & 15] as usize;
    let id = unicode_tables::PAGES[page][point & 63] as usize;
    let packed = unicode_tables::PROPERTIES[id];
    Property {
        class: Class::from_u8(packed as u8),
        flags: (packed >> 8) as u8,
    }
}

#[inline(always)]
fn resolved_property(codepoint: u32) -> Property {
    let mut value = property(codepoint);
    value.class = match value.class {
        Class::Ambiguous | Class::Sg | Class::Xx => Class::Al,
        Class::Sa if value.has(Property::COMBINING_CATEGORY) => Class::Cm,
        Class::Sa => Class::Al,
        Class::Cj => Class::Ns,
        class => class,
    };
    value
}

#[inline(always)]
fn normalize(value: Property, left: Option<Property>) -> Option<Property> {
    if matches!(value.class, Class::Cm | Class::Zwj) {
        if left
            .map(|left| accepts_combining(left.class))
            .unwrap_or(false)
        {
            return None;
        }
        return Some(Property {
            class: Class::Al,
            flags: 0,
        });
    }
    Some(value)
}

#[inline(always)]
fn accepts_combining(class: Class) -> bool {
    !matches!(
        class,
        Class::Bk | Class::Cr | Class::Lf | Class::Nl | Class::Sp | Class::Zw
    )
}

#[inline(always)]
fn significant_lookahead<I, It>(mut chars: It, mut left: Property) -> [Option<Property>; 2]
where
    It: Iterator<Item = (I, char)>,
{
    let mut found = [None, None];
    let mut count = 0;
    while count < found.len() {
        let (_, character) = match chars.next() {
            Some(value) => value,
            None => break,
        };
        let raw = resolved_property(character as u32);
        if let Some(value) = normalize(raw, Some(left)) {
            found[count] = Some(value);
            left = value;
            count += 1;
        }
    }
    found
}

#[inline(always)]
fn needs_lookahead(state: &State, right: Property) -> bool {
    right.class == Class::Qu
        || (state.last.class == Class::Sp && right.class == Class::Is)
        || (matches!(state.last.class, Class::Po | Class::Pr) && right.class == Class::Op)
        || (state.last.is_aksara_or_dotted() && right.is_aksara_or_dotted())
}

const PAIR_NO_BREAK: u8 = 0;
const PAIR_ALLOWED: u8 = 1;
const PAIR_MANDATORY: u8 = 2;
const PAIR_CONTEXT: u8 = 3;

static PAIR_BREAKS: [u8; 49 * 49] = build_pair_breaks();

#[inline(always)]
fn decide_pair(
    state: &mut State,
    right: Property,
    raw_zwj: bool,
) -> Result<Option<BreakOpportunity>, ()> {
    use BreakOpportunity::{Allowed, Mandatory};
    use Class::*;

    let left = state.last;
    if state.last_was_zwj {
        state.push(right, raw_zwj);
        return Ok(None);
    }
    if left.class == Sp {
        let opportunity = if matches!(right.class, Bk | Cr | Lf | Nl | Sp | Zw) {
            None
        } else if state.ends_with_spaced(Zw) {
            Some(Allowed)
        } else if matches!(right.class, Wj | Cl | Cp | Ex | Sy)
            || state.ends_with_spaced(Op)
            || state.pi_run
        {
            None
        } else if right.class == Qu {
            if right.has(Property::PF) {
                return Err(());
            }
            Some(Allowed)
        } else if right.class == Is {
            return Err(());
        } else if (right.class == Ns && (state.ends_with_spaced(Cl) || state.ends_with_spaced(Cp)))
            || (right.class == B2 && state.ends_with_spaced(B2))
        {
            None
        } else {
            Some(Allowed)
        };
        state.push(right, raw_zwj);
        return Ok(opportunity);
    }
    if matches!(left.class, Al | Hl) && matches!(right.class, Al | Hl) {
        state.push_common(right);
        return Ok(None);
    }
    if (left.class == Id && right.class == Id)
        || (matches!(left.class, H2 | H3) && matches!(right.class, H2 | H3))
    {
        state.push_common(right);
        return Ok(Some(Allowed));
    }
    let opportunity = match PAIR_BREAKS[left.class as usize * 49 + right.class as usize] {
        PAIR_NO_BREAK => None,
        PAIR_ALLOWED => Some(Allowed),
        PAIR_MANDATORY => Some(Mandatory),
        _ => return Err(()),
    };
    state.push(right, raw_zwj);
    Ok(opportunity)
}

const fn build_pair_breaks() -> [u8; 49 * 49] {
    let mut table = [PAIR_CONTEXT; 49 * 49];
    let mut left = 0;
    while left < 49 {
        let mut right = 0;
        while right < 49 {
            table[left * 49 + right] = pair_break(left as u8, right as u8);
            right += 1;
        }
        left += 1;
    }
    table
}

const fn pair_break(left: u8, right: u8) -> u8 {
    use Class::*;

    if pair_has_context(left, right) {
        return PAIR_CONTEXT;
    }
    if left == Bk as u8
        || (left == Cr as u8 && right != Lf as u8)
        || left == Lf as u8
        || left == Nl as u8
    {
        return PAIR_MANDATORY;
    }
    if (left == Cr as u8 && right == Lf as u8)
        || right == Bk as u8
        || right == Cr as u8
        || right == Lf as u8
        || right == Nl as u8
        || right == Sp as u8
        || right == Zw as u8
    {
        return PAIR_NO_BREAK;
    }
    if left == Zw as u8 {
        return PAIR_ALLOWED;
    }
    if left == Wj as u8 || right == Wj as u8 || left == Gl as u8 {
        return PAIR_NO_BREAK;
    }
    if right == Gl as u8
        && left != Sp as u8
        && left != Ba as u8
        && left != Hy as u8
        && left != Hh as u8
    {
        return PAIR_NO_BREAK;
    }
    if right == Cl as u8 || right == Cp as u8 || right == Ex as u8 || right == Sy as u8 {
        return PAIR_NO_BREAK;
    }
    if left == Op as u8 || right == Is as u8 {
        return PAIR_NO_BREAK;
    }
    if (right == Ns as u8 && (left == Cl as u8 || left == Cp as u8))
        || (left == B2 as u8 && right == B2 as u8)
    {
        return PAIR_NO_BREAK;
    }
    if left == Cb as u8 || right == Cb as u8 {
        return PAIR_ALLOWED;
    }
    if right == Ba as u8
        || right == Hh as u8
        || right == Hy as u8
        || right == Ns as u8
        || left == Bb as u8
        || (left == Sy as u8 && right == Hl as u8)
        || right == In as u8
    {
        return PAIR_NO_BREAK;
    }
    if ((left == Al as u8 || left == Hl as u8) && right == Nu as u8)
        || (left == Nu as u8 && (right == Al as u8 || right == Hl as u8))
        || (left == Pr as u8 && (right == Id as u8 || right == Eb as u8 || right == Em as u8))
        || ((left == Id as u8 || left == Eb as u8 || left == Em as u8) && right == Po as u8)
        || ((left == Pr as u8 || left == Po as u8) && (right == Al as u8 || right == Hl as u8))
        || ((left == Al as u8 || left == Hl as u8) && (right == Pr as u8 || right == Po as u8))
        || ((left == Po as u8 || left == Pr as u8) && right == Nu as u8)
        || ((left == Hy as u8 || left == Is as u8) && right == Nu as u8)
        || (left == Nu as u8 && right == Nu as u8)
    {
        return PAIR_NO_BREAK;
    }
    if (left == Jl as u8
        && (right == Jl as u8 || right == Jv as u8 || right == H2 as u8 || right == H3 as u8))
        || ((left == Jv as u8 || left == H2 as u8) && (right == Jv as u8 || right == Jt as u8))
        || ((left == Jt as u8 || left == H3 as u8) && right == Jt as u8)
        || ((left == Jl as u8
            || left == Jv as u8
            || left == Jt as u8
            || left == H2 as u8
            || left == H3 as u8)
            && right == Po as u8)
        || (left == Pr as u8
            && (right == Jl as u8
                || right == Jv as u8
                || right == Jt as u8
                || right == H2 as u8
                || right == H3 as u8))
        || ((left == Al as u8 || left == Hl as u8) && (right == Al as u8 || right == Hl as u8))
        || (left == Ap as u8 && (right == Ak as u8 || right == As as u8))
        || ((left == Ak as u8 || left == As as u8) && (right == Vf as u8 || right == Vi as u8))
        || (left == Is as u8 && (right == Al as u8 || right == Hl as u8))
        || (left == Eb as u8 && right == Em as u8)
    {
        return PAIR_NO_BREAK;
    }
    PAIR_ALLOWED
}

const fn pair_has_context(left: u8, right: u8) -> bool {
    use Class::*;

    left == Sp as u8
        || left == Qu as u8
        || right == Qu as u8
        || left == Hy as u8
        || left == Hh as u8
        || ((right == Po as u8 || right == Pr as u8)
            && (left == Nu as u8
                || left == Sy as u8
                || left == Is as u8
                || left == Cl as u8
                || left == Cp as u8))
        || (left == Sy as u8 && right == Nu as u8)
        || ((left == Po as u8 || left == Pr as u8) && right == Op as u8)
        || (left == Ri as u8 && right == Ri as u8)
        || right == Em as u8
        || ((left == Al as u8 || left == Hl as u8 || left == Nu as u8) && right == Op as u8)
        || (left == Cp as u8 && (right == Al as u8 || right == Hl as u8 || right == Nu as u8))
        || (left == Ap as u8 && right == Al as u8)
        || ((left == Ak as u8 || left == As as u8) && right == Al as u8)
        || (left == Al as u8
            && (right == Ak as u8 || right == As as u8 || right == Vf as u8 || right == Vi as u8))
        || ((left == Ak as u8 || left == As as u8) && (right == Ak as u8 || right == As as u8))
        || (left == Vi as u8 && (right == Ak as u8 || right == As as u8 || right == Al as u8))
        || left == Cm as u8
        || right == Cm as u8
        || left == Zwj as u8
        || right == Zwj as u8
        || left == Ambiguous as u8
        || right == Ambiguous as u8
        || left == Cj as u8
        || right == Cj as u8
        || left == Sa as u8
        || right == Sa as u8
        || left == Sg as u8
        || right == Sg as u8
        || left == Xx as u8
        || right == Xx as u8
}

#[inline(never)]
fn decide(state: &State, right: Property, next: [Option<Property>; 2]) -> Option<BreakOpportunity> {
    use BreakOpportunity::{Allowed, Mandatory};
    use Class::*;

    let left = state.last;
    if state.last_was_zwj {
        return None;
    }
    if left.class == Sp {
        if matches!(right.class, Bk | Cr | Lf | Nl | Sp | Zw) {
            return None;
        }
        if state.ends_with_spaced(Zw) {
            return Some(Allowed);
        }
        if matches!(right.class, Wj | Cl | Cp | Ex | Sy) {
            return None;
        }
        if state.ends_with_spaced(Op) || state.pi_run {
            return None;
        }
        if right.class == Qu {
            return if right.has(Property::PF)
                && next[0].map(|value| pf_suffix(value.class)).unwrap_or(true)
            {
                None
            } else {
                Some(Allowed)
            };
        }
        if right.class == Is {
            return if next[0].map(|value| value.class) == Some(Nu) {
                Some(Allowed)
            } else {
                None
            };
        }
        if (right.class == Ns && (state.ends_with_spaced(Cl) || state.ends_with_spaced(Cp)))
            || (right.class == B2 && state.ends_with_spaced(B2))
        {
            return None;
        }
        return Some(Allowed);
    }
    if matches!(left.class, Al | Hl) && matches!(right.class, Al | Hl) {
        return None;
    }
    if (left.class == Id && right.class == Id)
        || (matches!(left.class, H2 | H3) && matches!(right.class, H2 | H3))
    {
        return Some(Allowed);
    }
    match PAIR_BREAKS[left.class as usize * 49 + right.class as usize] {
        PAIR_NO_BREAK => return None,
        PAIR_ALLOWED => return Some(Allowed),
        PAIR_MANDATORY => return Some(Mandatory),
        _ => {}
    }
    if left.class == Bk || (left.class == Cr && right.class != Lf) || matches!(left.class, Lf | Nl)
    {
        return Some(Mandatory);
    }
    if left.class == Cr && right.class == Lf {
        return None;
    }
    if matches!(right.class, Bk | Cr | Lf | Nl) {
        return None;
    }
    if matches!(right.class, Sp | Zw) {
        return None;
    }
    if state.ends_with_spaced(Zw) {
        return Some(Allowed);
    }
    if matches!(left.class, Al | Hl) && matches!(right.class, Al | Hl) {
        return None;
    }
    if (matches!(left.class, Al | Hl) && right.class == Nu)
        || (left.class == Nu && matches!(right.class, Al | Hl | Nu))
    {
        return None;
    }
    if (left.class == Id && matches!(right.class, Al | Hl | Nu | Id))
        || (matches!(left.class, Al | Hl | Nu) && right.class == Id)
    {
        return Some(Allowed);
    }
    if left.class == Wj || right.class == Wj || left.class == Gl {
        return None;
    }
    if right.class == Gl && !matches!(left.class, Sp | Ba | Hy | Hh) {
        return None;
    }
    if matches!(right.class, Cl | Cp | Ex | Sy) {
        return None;
    }
    if state.ends_with_spaced(Op) || state.pi_run {
        return None;
    }
    if right.class == Qu
        && right.has(Property::PF)
        && next[0].map(|value| pf_suffix(value.class)).unwrap_or(true)
    {
        return None;
    }
    if right.class == Is {
        return None;
    }
    if right.class == Ns && (state.ends_with_spaced(Cl) || state.ends_with_spaced(Cp)) {
        return None;
    }
    if right.class == B2 && state.ends_with_spaced(B2) {
        return None;
    }
    if (right.class == Qu && !right.has(Property::PI))
        || (left.class == Qu && !left.has(Property::PF))
    {
        return None;
    }
    let right_followed_by_non_east_asian = next[0]
        .map(|value| !value.has(Property::EAST_ASIAN))
        .unwrap_or(true);
    if right.class == Qu && (!left.has(Property::EAST_ASIAN) || right_followed_by_non_east_asian) {
        return None;
    }
    if left.class == Qu
        && (!right.has(Property::EAST_ASIAN)
            || state
                .previous
                .map(|value| !value.has(Property::EAST_ASIAN))
                .unwrap_or(true))
    {
        return None;
    }
    if left.class == Cb || right.class == Cb {
        return Some(Allowed);
    }
    if matches!(left.class, Hy | Hh)
        && matches!(right.class, Al | Hl)
        && state
            .previous
            .map(|value| hyphen_prefix(value.class))
            .unwrap_or(true)
    {
        return None;
    }
    if matches!(right.class, Ba | Hh | Hy | Ns) || left.class == Bb {
        return None;
    }
    if matches!(left.class, Hy | Hh)
        && state.previous.map(|value| value.class) == Some(Hl)
        && right.class != Hl
    {
        return None;
    }
    if left.class == Sy && right.class == Hl {
        return None;
    }
    if right.class == In {
        return None;
    }
    if (matches!(left.class, Al | Hl) && right.class == Nu)
        || (left.class == Nu && matches!(right.class, Al | Hl))
    {
        return None;
    }
    if (left.class == Pr && matches!(right.class, Id | Eb | Em))
        || (matches!(left.class, Id | Eb | Em) && right.class == Po)
    {
        return None;
    }
    if (matches!(left.class, Pr | Po) && matches!(right.class, Al | Hl))
        || (matches!(left.class, Al | Hl) && matches!(right.class, Pr | Po))
    {
        return None;
    }
    if matches!(right.class, Po | Pr)
        && matches!(state.numeric, NumericState::Number | NumericState::Closed)
    {
        return None;
    }
    if matches!(left.class, Po | Pr)
        && (right.class == Nu
            || (right.class == Op
                && (next[0].map(|value| value.class) == Some(Nu)
                    || (next[0].map(|value| value.class) == Some(Is)
                        && next[1].map(|value| value.class) == Some(Nu)))))
    {
        return None;
    }
    if (matches!(left.class, Hy | Is) && right.class == Nu)
        || (state.numeric == NumericState::Number && right.class == Nu)
    {
        return None;
    }
    if (left.class == Jl && matches!(right.class, Jl | Jv | H2 | H3))
        || (matches!(left.class, Jv | H2) && matches!(right.class, Jv | Jt))
        || (matches!(left.class, Jt | H3) && right.class == Jt)
    {
        return None;
    }
    if (matches!(left.class, Jl | Jv | Jt | H2 | H3) && right.class == Po)
        || (left.class == Pr && matches!(right.class, Jl | Jv | Jt | H2 | H3))
    {
        return None;
    }
    if matches!(left.class, Al | Hl) && matches!(right.class, Al | Hl) {
        return None;
    }
    if (left.class == Ap && right.is_aksara_or_dotted())
        || (left.is_aksara_or_dotted() && matches!(right.class, Vf | Vi))
        || (state
            .previous
            .map(Property::is_aksara_or_dotted)
            .unwrap_or(false)
            && left.class == Vi
            && right.is_aksara_or_dotted())
        || (left.is_aksara_or_dotted()
            && right.is_aksara_or_dotted()
            && next[0].map(|value| value.class) == Some(Vf))
    {
        return None;
    }
    if left.class == Is && matches!(right.class, Al | Hl) {
        return None;
    }
    if (matches!(left.class, Al | Hl | Nu) && right.class == Op && !right.has(Property::EAST_ASIAN))
        || (left.class == Cp
            && !left.has(Property::EAST_ASIAN)
            && matches!(right.class, Al | Hl | Nu))
    {
        return None;
    }
    if left.class == Ri && right.class == Ri && state.regional_indicator_odd {
        return None;
    }
    if (left.class == Eb || left.has(Property::EXTENDED_PICTOGRAPHIC_CN)) && right.class == Em {
        return None;
    }
    Some(Allowed)
}

fn pi_prefix(class: Class) -> bool {
    matches!(
        class,
        Class::Bk
            | Class::Cr
            | Class::Lf
            | Class::Nl
            | Class::Op
            | Class::Qu
            | Class::Gl
            | Class::Sp
            | Class::Zw
    )
}

fn pf_suffix(class: Class) -> bool {
    matches!(
        class,
        Class::Sp
            | Class::Gl
            | Class::Wj
            | Class::Cl
            | Class::Qu
            | Class::Cp
            | Class::Ex
            | Class::Is
            | Class::Sy
            | Class::Bk
            | Class::Cr
            | Class::Lf
            | Class::Nl
            | Class::Zw
    )
}

fn hyphen_prefix(class: Class) -> bool {
    matches!(
        class,
        Class::Bk
            | Class::Cr
            | Class::Lf
            | Class::Nl
            | Class::Sp
            | Class::Zw
            | Class::Cb
            | Class::Gl
    )
}

fn is_safe_restart(value: Property) -> bool {
    unicode_tables::SAFE_RESTART_CLASSES & (1_u64 << value.class as u8) != 0
        && value.flags & unicode_tables::UNSAFE_RESTART_FLAGS == 0
}

#[cfg(test)]
#[path = "unit_tests.rs"]
mod unit_tests;
