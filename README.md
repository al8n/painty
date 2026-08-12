<div align="center">
<h1>painty</h1>
</div>
<div align="center">

Renders a diagnostic and its source text to a terminal, to HTML, or to an exportable model.

[<img alt="github" src="https://img.shields.io/badge/github-al8n/painty-8da0cb?style=for-the-badge&logo=Github" height="22">][Github-url]
<img alt="LoC" src="https://img.shields.io/endpoint?url=https%3A%2F%2Fgist.githubusercontent.com%2Fal8n%2F327b2a8aef9003246e45c6e47fe63937%2Fraw%2Fpainty" height="22">
[<img alt="Build" src="https://img.shields.io/github/actions/workflow/status/al8n/painty/ci.yml?logo=Github-Actions&style=for-the-badge" height="22">][CI-url]
[<img alt="codecov" src="https://img.shields.io/codecov/c/gh/al8n/painty?style=for-the-badge&logo=codecov" height="22">][codecov-url]

[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-painty-66c2a5?style=for-the-badge&labelColor=555555&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">][doc-url]
[<img alt="crates.io" src="https://img.shields.io/crates/v/painty?style=for-the-badge&logo=data:image/svg+xml;base64,PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iaXNvLTg4NTktMSI/Pg0KPCEtLSBHZW5lcmF0b3I6IEFkb2JlIElsbHVzdHJhdG9yIDE5LjAuMCwgU1ZHIEV4cG9ydCBQbHVnLUluIC4gU1ZHIFZlcnNpb246IDYuMDAgQnVpbGQgMCkgIC0tPg0KPHN2ZyB2ZXJzaW9uPSIxLjEiIGlkPSJMYXllcl8xIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHhtbG5zOnhsaW5rPSJodHRwOi8vd3d3LnczLm9yZy8xOTk5L3hsaW5rIiB4PSIwcHgiIHk9IjBweCINCgkgdmlld0JveD0iMCAwIDUxMiA1MTIiIHhtbDpzcGFjZT0icHJlc2VydmUiPg0KPGc+DQoJPGc+DQoJCTxwYXRoIGQ9Ik0yNTYsMEwzMS41MjgsMTEyLjIzNnYyODcuNTI4TDI1Niw1MTJsMjI0LjQ3Mi0xMTIuMjM2VjExMi4yMzZMMjU2LDB6IE0yMzQuMjc3LDQ1Mi41NjRMNzQuOTc0LDM3Mi45MTNWMTYwLjgxDQoJCQlsMTU5LjMwMyw3OS42NTFWNDUyLjU2NHogTTEwMS44MjYsMTI1LjY2MkwyNTYsNDguNTc2bDE1NC4xNzQsNzcuMDg3TDI1NiwyMDIuNzQ5TDEwMS44MjYsMTI1LjY2MnogTTQzNy4wMjYsMzcyLjkxMw0KCQkJbC0xNTkuMzAzLDc5LjY1MVYyNDAuNDYxbDE1OS4zMDMtNzkuNjUxVjM3Mi45MTN6IiBmaWxsPSIjRkZGIi8+DQoJPC9nPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPC9zdmc+DQo=" height="22">][crates-url]
[<img alt="crates.io" src="https://img.shields.io/crates/d/painty?color=critical&logo=data:image/svg+xml;base64,PD94bWwgdmVyc2lvbj0iMS4wIiBzdGFuZGFsb25lPSJubyI/PjwhRE9DVFlQRSBzdmcgUFVCTElDICItLy9XM0MvL0RURCBTVkcgMS4xLy9FTiIgImh0dHA6Ly93d3cudzMub3JnL0dyYXBoaWNzL1NWRy8xLjEvRFREL3N2ZzExLmR0ZCI+PHN2ZyB0PSIxNjQ1MTE3MzMyOTU5IiBjbGFzcz0iaWNvbiIgdmlld0JveD0iMCAwIDEwMjQgMTAyNCIgdmVyc2lvbj0iMS4xIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHAtaWQ9IjM0MjEiIGRhdGEtc3BtLWFuY2hvci1pZD0iYTMxM3guNzc4MTA2OS4wLmkzIiB3aWR0aD0iNDgiIGhlaWdodD0iNDgiIHhtbG5zOnhsaW5rPSJodHRwOi8vd3d3LnczLm9yZy8xOTk5L3hsaW5rIj48ZGVmcz48c3R5bGUgdHlwZT0idGV4dC9jc3MiPjwvc3R5bGU+PC9kZWZzPjxwYXRoIGQ9Ik00NjkuMzEyIDU3MC4yNHYtMjU2aDg1LjM3NnYyNTZoMTI4TDUxMiA3NTYuMjg4IDM0MS4zMTIgNTcwLjI0aDEyOHpNMTAyNCA2NDAuMTI4QzEwMjQgNzgyLjkxMiA5MTkuODcyIDg5NiA3ODcuNjQ4IDg5NmgtNTEyQzEyMy45MDQgODk2IDAgNzYxLjYgMCA1OTcuNTA0IDAgNDUxLjk2OCA5NC42NTYgMzMxLjUyIDIyNi40MzIgMzAyLjk3NiAyODQuMTYgMTk1LjQ1NiAzOTEuODA4IDEyOCA1MTIgMTI4YzE1Mi4zMiAwIDI4Mi4xMTIgMTA4LjQxNiAzMjMuMzkyIDI2MS4xMkM5NDEuODg4IDQxMy40NCAxMDI0IDUxOS4wNCAxMDI0IDY0MC4xOTJ6IG0tMjU5LjItMjA1LjMxMmMtMjQuNDQ4LTEyOS4wMjQtMTI4Ljg5Ni0yMjIuNzItMjUyLjgtMjIyLjcyLTk3LjI4IDAtMTgzLjA0IDU3LjM0NC0yMjQuNjQgMTQ3LjQ1NmwtOS4yOCAyMC4yMjQtMjAuOTI4IDIuOTQ0Yy0xMDMuMzYgMTQuNC0xNzguMzY4IDEwNC4zMi0xNzguMzY4IDIxNC43MiAwIDExNy45NTIgODguODMyIDIxNC40IDE5Ni45MjggMjE0LjRoNTEyYzg4LjMyIDAgMTU3LjUwNC03NS4xMzYgMTU3LjUwNC0xNzEuNzEyIDAtODguMDY0LTY1LjkyLTE2NC45MjgtMTQ0Ljk2LTE3MS43NzZsLTI5LjUwNC0yLjU2LTUuODg4LTMwLjk3NnoiIGZpbGw9IiNmZmZmZmYiIHAtaWQ9IjM0MjIiIGRhdGEtc3BtLWFuY2hvci1pZD0iYTMxM3guNzc4MTA2OS4wLmkwIiBjbGFzcz0iIj48L3BhdGg+PC9zdmc+&style=for-the-badge" height="22">][crates-url]
<img alt="license" src="https://img.shields.io/badge/License-Apache%202.0/MIT-blue.svg?style=for-the-badge&fontColor=white&logoColor=f5c076&logo=data:image/svg+xml;base64,PCFET0NUWVBFIHN2ZyBQVUJMSUMgIi0vL1czQy8vRFREIFNWRyAxLjEvL0VOIiAiaHR0cDovL3d3dy53My5vcmcvR3JhcGhpY3MvU1ZHLzEuMS9EVEQvc3ZnMTEuZHRkIj4KDTwhLS0gVXBsb2FkZWQgdG86IFNWRyBSZXBvLCB3d3cuc3ZncmVwby5jb20sIFRyYW5zZm9ybWVkIGJ5OiBTVkcgUmVwbyBNaXhlciBUb29scyAtLT4KPHN2ZyBmaWxsPSIjZmZmZmZmIiBoZWlnaHQ9IjgwMHB4IiB3aWR0aD0iODAwcHgiIHZlcnNpb249IjEuMSIgaWQ9IkNhcGFfMSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIiB4bWxuczp4bGluaz0iaHR0cDovL3d3dy53My5vcmcvMTk5OS94bGluayIgdmlld0JveD0iMCAwIDI3Ni43MTUgMjc2LjcxNSIgeG1sOnNwYWNlPSJwcmVzZXJ2ZSIgc3Ryb2tlPSIjZmZmZmZmIj4KDTxnIGlkPSJTVkdSZXBvX2JnQ2FycmllciIgc3Ryb2tlLXdpZHRoPSIwIi8+Cg08ZyBpZD0iU1ZHUmVwb190cmFjZXJDYXJyaWVyIiBzdHJva2UtbGluZWNhcD0icm91bmQiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz4KDTxnIGlkPSJTVkdSZXBvX2ljb25DYXJyaWVyIj4gPGc+IDxwYXRoIGQ9Ik0xMzguMzU3LDBDNjIuMDY2LDAsMCw2Mi4wNjYsMCwxMzguMzU3czYyLjA2NiwxMzguMzU3LDEzOC4zNTcsMTM4LjM1N3MxMzguMzU3LTYyLjA2NiwxMzguMzU3LTEzOC4zNTcgUzIxNC42NDgsMCwxMzguMzU3LDB6IE0xMzguMzU3LDI1OC43MTVDNzEuOTkyLDI1OC43MTUsMTgsMjA0LjcyMywxOCwxMzguMzU3UzcxLjk5MiwxOCwxMzguMzU3LDE4IHMxMjAuMzU3LDUzLjk5MiwxMjAuMzU3LDEyMC4zNTdTMjA0LjcyMywyNTguNzE1LDEzOC4zNTcsMjU4LjcxNXoiLz4gPHBhdGggZD0iTTE5NC43OTgsMTYwLjkwM2MtNC4xODgtMi42NzctOS43NTMtMS40NTQtMTIuNDMyLDIuNzMyYy04LjY5NCwxMy41OTMtMjMuNTAzLDIxLjcwOC0zOS42MTQsMjEuNzA4IGMtMjUuOTA4LDAtNDYuOTg1LTIxLjA3OC00Ni45ODUtNDYuOTg2czIxLjA3Ny00Ni45ODYsNDYuOTg1LTQ2Ljk4NmMxNS42MzMsMCwzMC4yLDcuNzQ3LDM4Ljk2OCwyMC43MjMgYzIuNzgyLDQuMTE3LDguMzc1LDUuMjAxLDEyLjQ5NiwyLjQxOGM0LjExOC0yLjc4Miw1LjIwMS04LjM3NywyLjQxOC0xMi40OTZjLTEyLjExOC0xNy45MzctMzIuMjYyLTI4LjY0NS01My44ODItMjguNjQ1IGMtMzUuODMzLDAtNjQuOTg1LDI5LjE1Mi02NC45ODUsNjQuOTg2czI5LjE1Miw2NC45ODYsNjQuOTg1LDY0Ljk4NmMyMi4yODEsMCw0Mi43NTktMTEuMjE4LDU0Ljc3OC0zMC4wMDkgQzIwMC4yMDgsMTY5LjE0NywxOTguOTg1LDE2My41ODIsMTk0Ljc5OCwxNjAuOTAzeiIvPiA8L2c+IDwvZz4KDTwvc3ZnPg==" height="22">

</div>

## Status

**Layer 2, a terminal renderer in four styles, and HTML.** Resolution has landed: a diagnostic and
the source text it points into go in, and lines, character columns, excerpts and the lines a
multi-line span is drawn on come out. The terminal renderer draws them with box characters and ANSI
colour, in whichever of `rustc`, `miette`, `ariadne` and `codespan`'s idioms the caller picks. The
HTML renderer writes the same thing as elements and CSS classes, with every byte of the caller's
text escaped; it is a **peer** of the terminal rather than another of its styles, because the seam
the styles are written against sits below the conversion into display cells and HTML has no cell.
The model export is declared as a feature and is not written.

Nothing here is published to crates.io.

### Caller text never steers the terminal

Diagnostic text is caller-supplied and so is the source it points into, so every control character
in either — the message, the code, the origin, a label, the excerpt — is replaced by a visible
stand-in before it is written. An ESC reads as `␛`, DEL as `␡`, and C1, which has no pictures, as
the replacement character.

Not cosmetic. `\x1b[38;5;196m` sitting in a source file would otherwise colour the rest of the
frame, and it would do it under `ColorCapability::None`, which is the one guarantee the capability
gate exists to make; U+009B is a single-byte CSI on terminals that honour C1, and a bare newline
needs no escape at all to break the frame. Substituted rather than dropped, because a reader has to
be able to see that something was there, and every stand-in is one cell wide so no marker moves.

## Overview

A diagnostic is worth more when it does not know how it will be displayed. Producers —
[smear](https://github.com/al8n/smear), [pql](https://github.com/al8n/pql), and
[tokora](https://github.com/al8n/tokora)'s coming SQL, Yul and Solidity frontends — describe what
went wrong as structure: a code, a severity, positions, labels, a help line. `painty` is the other
half of that split: give it such a description and the text it points into, and it produces
something a human, a browser or a native UI can read.

Existing renderers do not fit that shape. `miette` and `ariadne` both require `std`, both own their
strings, and both bake a rendering strategy into the error type — which is the coupling worth
avoiding in the first place.

```rust
use painty::{Diagnostic, Label, Location, Severity, Source, Span};

let text = "type Widget {\n  width: Int\n  width: Int\n}\n";
let source = Source::new(text);

let message = format_args!("`{}` is defined twice", "width");
let labels = [Label::new(Location::new(0, Span::new(16, 21)), "first defined here")];

let diagnostic = Diagnostic::new(
  "mylang::schema::duplicate-field",
  Severity::Error,
  &message,
  Location::new(0, Span::new(29, 34)),
)
.with_labels(&labels)
.with_help("rename one of the two definitions");

let region = source.resolve(diagnostic.primary().span().unwrap());
assert_eq!(region.text(), "width");
assert_eq!((region.start().line(), region.start().column()), (3, 3));
```

## The seam is data, not a trait

`painty` takes a `Diagnostic`, a borrowed view it defines itself over nothing but `core`. It does
not ask a caller to implement a trait, and it does not require a diagnostic contract from any
particular crate.

That is the point rather than an omission. A renderer must not force a GraphQL library on a project
that wanted an underline drawn — and the same argument does not stop at GraphQL. A Rust project
with its own error type should not have to adopt a parser-combinator library either. So the
`tokora` feature is an *adapter*, one small module of conversions, and the crate is useful with
nothing enabled at all.

The alternative — declaring a trait here and blanket-implementing it for tokora's — is rejected
deliberately. Two structurally identical traits maintained in two repositories drift, and a trait
that has fallen behind its twin keeps compiling. A conversion function does not.

The claim is checked rather than stated: the consumer suites in `tests/` are written against a
hand-rolled local error type, they run in a build whose dependency graph is empty, and the
`no-tokora` CI job reads `cargo tree` over every other configuration to prove the dependency is not
quietly there.

## Three layers

The layer boundary is where the design does its real work, and it is not the obvious one. A
terminal renderer and an HTML renderer both emit text, so their shared work could live inside
either of them. A SwiftUI or Flutter front end emits a widget tree, on the far side of an FFI
boundary, in another language — it cannot consume a string, it needs the structure. So resolution
becomes a public layer of its own rather than an implementation detail of the text renderers:

```text
Layer 1 — the description     a Diagnostic: code, severity, primary position, labels,
                              result path, help. Borrowed data, no trait, no dependency.

Layer 2 — resolution          byte offset -> (line, column); excerpt extraction; ordering;
                              the lines a region is drawn on
                              borrows from the source, still allocation-free
                              THE PUBLIC DATA MODEL — landed

Layer 3 — outputs             terminal (ANSI) — landed, four styles
                              HTML (elements and classes) — landed, a PEER of the terminal
                              model export for native UIs — declared as a feature; not written
```

The two text outputs are peers rather than a renderer and a style of it, and that is a finding
rather than a preference. The seam the four terminal styles are written against sits *below* the
character-column-to-display-column conversion: one of its hooks returns a `char` to stand in a cell
and four more position a mark by display column, and HTML has no cell. It also draws the line in
the wrong place — a terminal style marks a row from *underneath*, and an HTML mark is an element
*inside* the row, so the source row itself changes hands.

What the two share is **layer 2**, which is stated in lines and byte spans and which every medium
has. Not the terminal's plan: that is private to its own module, it is built by putting questions
to a style and a cell budget, and it is four `Vec`s in a renderer that has no allocator.

Layer 2 earns its keep twice. Beyond serving outputs that are not text, it is what makes the hard
part assertable structurally instead of by snapshot: every reported column lies within its line's
width, an excerpt re-sliced from the source by the reported offsets equals the span it came from,
and a multi-line region opens and closes on the lines it actually covers. Golden files are then
left covering only appearance, where re-blessing one is cheap and correct.

Grouping labels by line and eliding distant ones are layer 2's too, and are deliberately not here
yet: whether elision stays allocation-free is a measurement, and it is taken when the first
renderer can say what shape it needs.

## Numeric widths

A published model's integer widths cannot be changed later without breaking every consumer, and
this one is meant to cross a C ABI as well as a Rust API. So the width of a numeric member is not
a local choice — it follows from what the number *is*, by five rules read in order. The first that
matches wins, and the last matches everything, so there is no member the rule fails to place.

| # | what the number is | width | why |
| - | ------------------ | ----- | --- |
| 1 | a line or column in painty's own geometry, resolved **or rendered** — including a count of lines, which is the last line's own number | `u64` | it crosses a C ABI, so not `usize`; and it must not imply a ceiling the domain does not have, so not `u32` |
| 2 | an ordinal in data the **producer** built, that painty neither computes nor bounds | `u64` | same ceiling argument, and there is no protocol cap to point at instead |
| 3 | a key into a structure painty does not own and never computes | that contract's width | it has to round-trip; matching the width is what makes it lossless in both directions |
| 4 | a value painty itself **emits into a wire format** that fixes its width | that format's width | the format is not negotiable and the value has to be legal in it |
| 5 | anything else — an index or a count of things in memory | `usize` | exactly as `slice::len` is |

Worked through the whole public surface, that gives:

- **rule 1** — `Position::line`, `Position::column`, `Line::number`, `Line::char_count`,
  `Line::column_at`, `Source::line_count`, `Source::line`, `Region::line_count`,
  `RegionLine::columns`, and every member of `LineCells` — `column_at`, `width`, `columns_for`,
  `cells_between`, `default_tab_width`, `max_tab_width` — display columns being painty's own
  geometry just as character columns are;
- **rule 2** — `PathSegment::Index`, a position in a *result* the producer assembled;
- **rule 3** — `Location::source`, a `u32` index into the caller's own list of inputs, which is
  what every producer of one already spells it;
- **rule 4** — `Color::Ansi256`, `Color::Rgb`, `Ansi16::{index, from_index, to_rgb}`: a colour
  channel painty writes into an SGR escape sequence, which fixes it at one byte;
- **rule 5** — `Position::offset`, `Span::{new, empty, start, end, len, contains}`,
  `LineBreak::byte_len`, and `Source::{len, line_at, position}`.

The adapter's overflow reports used to be here, as exact `usize` counts. They are booleans now and
have left the numeric surface entirely: an exact count of what did not fit can only be had by
looking at everything that did not fit, and those are answers from a caller-implemented trait, so
the count was buying an unbounded walk through somebody else's code to fill a four-element array.
`painty::tokora::Adapted` documents the trade.

Rule 4 was added when the terminal renderer arrived, and it is the residual this document had been
carrying — *the rule set itself being wrong* — actually firing. A colour channel is not an ordinal,
and rule 3 excludes it on its own terms because painty **does** compute it: narrowing `Rgb → 256 →
16` is painty's arithmetic. The catch-all then claimed it and gave a pointer-sized colour channel,
which is absurd. The alternative considered was widening rule 3 from *authorship of the value* to
*ownership of the contract*, which covers both with one rule instead of two; it was rejected because
dropping rule 3's "never computes" clause lets it also claim every byte offset — Rust's slicing
contract fixes those at `usize` — which would leave rule 3 swallowing rule 5 and the procedure with
nothing to discriminate on. Five rules that each decide something beat four where one decides
everything.

Rule 1's wording widened at the same time, from *the resolved model* to *painty's own geometry,
resolved or rendered*, so that a renderer's display column is placed. Checked against all
twenty-four members that existed before: none is re-placed, and the near misses — `Source::line_at`,
`LineBreak::byte_len`, `PathSegment::Index`, `Location::source`, `Position::offset` — are byte
offsets and foreign indices rather than lines or columns.

Two consequences worth stating, because they are why the rules are ordered rather than merely
listed. A line count is *both* a count of things in memory and a line ordinal; rule 1 comes first,
so it is a `u64`. And nothing on the path a position is built by may saturate or cast: a clamped
ordinal is indistinguishable from a real one, which is a number that lies rather than one that
fails.

**A parameter's width is as frozen as a return's**, so both are pinned — and so is the lifetime a
borrow comes back on. Three things enforce this, and it is worth knowing which one decides.

`ci/public_numeric_surface.py`, run by the `public-surface` CI job, **is the decider**. It asks
rustdoc for the *resolved* public API and recurses over it looking for primitive integers, so it
names no syntactic position and there is no case in it to forget: a width in a const-generic
parameter, behind a type alias, or produced by a macro expansion is found by the same three lines
that find one in a return type. Every member above is recorded there with its widths.

`tests/numeric_widths.rs` is a **fast early warning** that says so in its own header, and it is
also where each member is pinned by function pointer under the rule that placed it — so the rule is
a fact in the code, and a member carrying a width its rule does not give fails mechanically.

`tests/source_borrows.rs` covers the other half: everything resolution returns borrows the source
text rather than the `&Source` it came through.

What is left is a member placed under a rule that gives the *same* width — rules 1 and 2 both give
`u64`, and they differ in where the number came from rather than in what it is — and the rule set
itself being wrong. No check reads intent. That is what the ordering above is for, and what review
is for.

Two of those are associated functions where a constant would read more naturally —
`LineCells::default_tab_width` and `LineCells::max_tab_width`. A `pub const` cannot be pinned by
ascribing a function pointer, and the choice was between a second pinning mechanism for two members
and an API shape the one mechanism already covers. The API moved.

## Features

Every output is independently selectable, and the default configuration selects none of them — the
crate is `no_std` and dependency-free until a caller asks for something.

| feature     | implies | pulls in                   | what it turns on                                    |
| ----------- | ------- | -------------------------- | --------------------------------------------------- |
| *(default)* | —       | —                          | layer 2: resolution, `no_std`, no dependencies      |
| `std`       | —       | —                          | anything needing the standard library               |
| `terminal`  | `std`   | `unicode-width`, `unicode-segmentation`, `anstyle` | the terminal renderer: cell arithmetic, colour detection, ANSI |
| `html`      | —       | —                          | the HTML renderer: escaping and CSS classes, still `no_std` and allocation-free |
| `model`     | —       | —                          | a stable C-ABI export of the resolved model         |
| `tokora`    | —       | `tokora`                   | an adapter from `tokora::diagnostic::Diagnose`      |

```toml
[dependencies]
painty = { version = "0", features = ["terminal"] }
```

`unicode-width` is taken rather than hand-narrowed because correct terminal alignment is impossible
without display width, and owning a Unicode table means owning a class of alignment bug for no
gain. `unicode-segmentation` is taken for a sharper version of the same reason: where a *placement
unit* ends is UAX#29 grapheme segmentation, and two rounds of review found two different home-grown
answers that each inferred it from incremental prefix width — the second placing every marker after
a variation-selector emoji one cell early. A rule that has to be tuned a third time is a wrong
model, so the question goes to the crate that implements the standard, exactly as `syn` and not a
line scanner decides what a public item is. `anstyle` is what `clap` and `cargo` already use, so a
consumer's `--color` flag and this crate's theme interoperate without a conversion layer. All three
sit behind `terminal`; the HTML and model outputs pull in none of them.

`html` implies nothing at all, which is load-bearing rather than tidy: CI builds it for
`thumbv6m-none-eabi`, so the renderer has no `std`, no `alloc` and no heap. That is why it does not
consume the terminal's plan — a plan is four `Vec`s — and orders the caller's labels with selection
scans over their own byte offsets instead. The price is stated where it is paid: `O(k²)` integer
comparisons in the label count, against the terminal's `O(k log k)`, measured at 31 ms for a
thousand labels. What the two renderers do share is layer 2 and the elision rule, which is one
function both call rather than one rule each keeps — the first thing two copies of it did was
disagree about which lines a reader is shown.

### The placement model

A line is its sequence of UAX#29 extended grapheme clusters, after sanitization. Each cluster
occupies exactly `unicode-width`'s width of that cluster **measured in isolation**. A byte offset's
column is one plus the cells of the whole clusters before it, and a span widens outward to cluster
boundaries.

`unicode-width` also applies rules *across* cluster boundaries — Arabic lam followed by alef scores
1 for the pair where the clusters score 1 + 1 — and painty **rejects those by specification**. Not
an omission: no cursor-addressable terminal can implement them. A grid device must have a definite
cursor position between any two characters it receives, `CSI 6n` can be issued between the lam and
the alef, and the lam's cluster has closed before the alef arrives. For the pair to occupy one cell
the alef would have to advance zero cells into a cell the terminal may already have reported past.
Every real terminal advances two. The cluster boundary is the maximum lookahead a cursor-addressable
device can hold without contradicting its own cursor reports, which is why it is the unit.

So painty promises exact cell alignment on a grapheme-aware terminal, and declines four things:
agreement with whole-string width (six families differ on Unicode 17 data, each pinned in the tests
at *both* values); visual alignment under bidi reordering, since columns are logical; font shaping,
so underlining half a ligature marks half the span — deliberately, because those are two addressable
source positions; and legacy per-codepoint cell counts, where a wcwidth-era terminal gives a ZWJ
emoji sequence more cells than its cluster width.

`tokora` is taken with `default-features = false`, so it stays `no_std` and brings only the
diagnostic contract the adapter reads — `--features tokora` is one of the bare-metal legs the
`no-std` job builds.

SwiftUI and Flutter bindings are deliberately not features. They are separate repositories
consuming the `model` export, so this crate never needs to know Swift or Dart exist.

## Minimum supported Rust version

`1.95`, matching `tokora` — the crate whose diagnostic contract `painty`'s optional adapter reads.
Keeping the floor no lower than the contract's is what makes it a floor a caller can stand on. The
`msrv` CI job reads `rust-version` out of `Cargo.toml` rather than hardcoding a toolchain, so the
declared value and the tested value cannot drift.

Raising the floor is not treated as a breaking change.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

[Github-url]: https://github.com/al8n/painty/
[CI-url]: https://github.com/al8n/painty/actions/workflows/ci.yml
[codecov-url]: https://app.codecov.io/gh/al8n/painty
[doc-url]: https://docs.rs/painty
[crates-url]: https://crates.io/crates/painty
