# uax14-linebreak

`uax14-linebreak` reports line break opportunities for Unicode 17.0.0 text. It implements the default rules from Unicode Standard Annex #14 revision 55.

The crate accepts UTF-8 strings or iterators of indexed characters. It works in `no_std` programs and has no dependencies. Property lookup and line breaking use static tables and do not allocate.

```rust
use uax14_linebreak::{linebreaks, BreakOpportunity};

let breaks: Vec<_> = linebreaks("Hello world").collect();
assert_eq!(
    breaks,
    [
        (6, BreakOpportunity::Allowed),
        (11, BreakOpportunity::Mandatory),
    ]
);
```

Each index is the byte position of the character that starts the next line. Nonempty input ends with a mandatory opportunity at `text.len()`. Empty input yields no opportunities.

Use `linebreaks_iter` when indices come from another text source. The index type only needs `Clone`.

```rust
use uax14_linebreak::{linebreaks_iter, BreakOpportunity};

let chars = [("first", 'a'), ("space", ' '), ("second", 'b')];
let breaks: Vec<_> = linebreaks_iter(chars.iter().cloned(), "end").collect();
assert_eq!(
    breaks,
    [
        ("second", BreakOpportunity::Allowed),
        ("end", BreakOpportunity::Mandatory),
    ]
);
```

`split_at_safe` finds the last position where the second slice can start with fresh line-breaking state. This supports chunked processing without carrying state across the selected boundary.

The conformance suite covers all 19,338 cases in the Unicode 17.0.0 line break test data with no skipped cases. It also checks iterator equivalence, restart-point regressions, Unicode version changes, and zero allocation across the conformance corpus.

## License

The package license expression is `MIT AND Unicode-3.0`. The Rust code is available under the MIT License, in [LICENSE](LICENSE). The Unicode property tables and conformance data derive from the Unicode Character Database and are available under the Unicode License v3, in [LICENSE-UNICODE](LICENSE-UNICODE).
