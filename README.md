<div align="center">
<h1>painty</h1>
</div>
<div align="center">

Renders a diagnostic and its source text to a terminal or HTML.

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

Resolution has landed: a diagnostic and the source text it points into go in, and lines, character
columns, excerpts and the lines a multi-line span covers come out. On top of it sit the terminal
renderer in four styles (`rustc`, `miette`, `ariadne` and `codespan`), an HTML renderer, and an SVG
surface for the terminal render. The `model` feature, an export of the resolved model for native
UIs, is reserved and implements nothing.

### Caller text never steers the terminal

Diagnostic text and the source it points into are caller-supplied, so every control character in
either — the message, the code, the origin, a label, the help line, the excerpt — is replaced by a
visible stand-in before it is written: ESC reads as `␛`, DEL as `␡`, and C1, which has no picture,
as the replacement character. (A tab in source text is expanded to its tab stop instead.) This holds
in every style and at every colour capability, including `ColorCapability::None`, which promises no
escape sequences at all. A stand-in is one cell wide, so no marker moves.

## Overview

A diagnostic is worth more when it does not know how it will be displayed. A producer — a parser, a
validator, a linter — describes what went wrong as structure: a code, a severity, positions, labels
and a help line. `painty` is the other half of that split: give it such a description and the text
it points into, and it produces something a human, a browser or a native UI can read.

`miette` and `ariadne` are renderers too, but both require `std`, both own their strings, and both
tie how a diagnostic is drawn to the type that carries it. `painty` takes plain borrowed data over
`core`, so the producer does not choose a renderer and the renderer does not choose the producer.

## Quick start

```toml
[dependencies]
painty = { version = "0.0.0", features = ["terminal", "html"] }
```

### Describe a diagnostic

A `Diagnostic` is borrowed data: a code, a severity, a message, a primary position, and optionally
labels and a help line. A `Location` is an input number and a byte span (or `Location::entire`, for
a whole input); the number indexes the list of inputs a renderer is given. Resolving a span against
a `Source` needs no feature.

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

### Render it to a terminal

With the `terminal` feature, `Terminal::render` writes a diagnostic against the inputs its locations
point into. `Terminal::plain()` draws in the `rustc` style with no colour.

```rust
# #[cfg(feature = "terminal")] {
use painty::{Diagnostic, Input, Location, Severity, Source, Span, terminal::Terminal};

let text = "type Widget {\n  width: Int\n  width: Int\n}\n";
let message = "`width` is defined twice";
let diagnostic = Diagnostic::new(
  "mylang::schema::duplicate-field",
  Severity::Error,
  &message,
  Location::new(0, Span::new(29, 34)),
)
.with_primary_label("redefined here")
.with_help("rename one of the two definitions");

let inputs = [Input::new(Source::new(text)).with_origin("widget.graphql")];

let mut out = String::new();
Terminal::plain().render(&diagnostic, &inputs, &mut out).unwrap();
assert!(out.starts_with("error[mylang::schema::duplicate-field]: `width` is defined twice\n"));
assert!(out.contains("  |   ^^^^^ redefined here\n"));

// The same diagnostic in another style.
let mut boxed = String::new();
Terminal::plain().like_miette().render(&diagnostic, &inputs, &mut boxed).unwrap();
assert!(boxed.contains("┗━━ redefined here"));
# }
```

```text
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> widget.graphql:3:3
  |
3 |   width: Int
  |   ^^^^^ redefined here
  |
  = help: rename one of the two definitions
```

`like_rustc()`, `like_miette()`, `like_ariadne()` and `like_codespan()` pick the style. A style
changes glyphs and layout; which source is marked is the same in all four.

Colour is opt-in. `Terminal::plain()` writes no escape sequences, and `with_capability` turns on as
much colour as the output can carry. `ColorChoice` resolves the usual `NO_COLOR`, `CLICOLOR_FORCE`,
`CLICOLOR`, `TERM` and `COLORTERM` into a `ColorCapability`, so a `--color` flag maps straight onto
it.

```rust
# #[cfg(feature = "terminal")] {
use painty::terminal::{CapturedEnvironment, ColorCapability, ColorChoice, Environment, Terminal};

let captured = CapturedEnvironment::from_process();
let environment = Environment::from_process(&std::io::stderr(), &captured);
let terminal = Terminal::plain().with_capability(ColorChoice::Auto.resolve(&environment));

// A caller's `--color=never` is not overruled by the environment.
assert_eq!(ColorChoice::Never.resolve(&environment), ColorCapability::None);
# }
```

### Render it as HTML

With the `html` feature, `Html::render` takes the same arguments and writes elements and CSS
classes instead of cells. The markup carries `painty-` classes and no styling, so the page that
embeds it decides what it looks like.

```rust
# #[cfg(feature = "html")] {
use painty::{Diagnostic, Input, Location, Severity, Source, Span, html::Html};

let text = "type Widget {\n  width: Int\n  width: Int\n}\n";
let message = "`width` is defined twice";
let diagnostic = Diagnostic::new(
  "mylang::schema::duplicate-field",
  Severity::Error,
  &message,
  Location::new(0, Span::new(29, 34)),
)
.with_primary_label("redefined here")
.with_help("rename one of the two definitions");

let mut out = String::new();
Html::new()
  .render(&diagnostic, &[Input::new(Source::new(text))], &mut out)
  .unwrap();

assert!(out.contains(r#"<span class="painty-code">mylang::schema::duplicate-field</span>"#));
assert!(out.contains(r#"<span class="painty-covered painty-primary">width</span>"#));
assert!(out.contains(r#"<p class="painty-help">rename one of the two definitions</p>"#));
# }
```

The caller's text is escaped, so none of it reaches the page as markup. With the `svg` feature,
`Terminal::render_svg` takes the arguments `render` does and writes the same render as an SVG
image.

## Rendering tokora diagnostics

With the `tokora` feature, `painty::tokora::adapt` reads a `&dyn tokora::diagnostic::Diagnose`
(tokora 0.13) into a `Diagnostic`, so any front end whose errors implement that contract renders
through the same renderers. `painty` allocates nothing, so the labels and the result path land in
buffers you supply, usually arrays on the stack.

```rust
# #[cfg(all(feature = "tokora", feature = "terminal"))] {
use painty::{Input, Label, Location, PathSegment, Source, terminal::Terminal, tokora::adapt};
use tokora::diagnostic::Diagnose;

fn report(error: &dyn Diagnose, text: &str) -> (String, bool) {
  let mut labels = [Label::new(Location::entire(0), ""); 4];
  let mut path: [PathSegment<'_>; 4] = [PathSegment::Index(0); 4];
  let adapted = adapt(error, &mut labels, &mut path);

  let mut out = String::new();
  Terminal::plain()
    .render(&adapted.diagnostic(), &[Input::new(Source::new(text))], &mut out)
    .expect("a `String` never fails to be written to");
  (out, adapted.is_complete())
}
# }
```

A diagnostic carries nought to five labels in practice, so a small array is the expected shape.
Anything past the end of a buffer is dropped, and the adapter says so rather than dropping it
silently: `Adapted::is_complete` is `false` when either buffer was too small, and
`labels_dropped` and `path_segments_dropped` say which. A caller with no labels or path passes
`&mut []`.

The adapter's work is bounded by your buffers and not by the diagnostic: it calls the label accessor
at most `labels.len() + 1` times and the path accessor at most `path.len() + 1`, and it never calls
`Diagnose::labels` or `Diagnose::path_segments`, because those counts come from caller code and are
not trusted. That is also why it reports whether something was dropped rather than how many. A
severity tokora adds later maps to `Severity::Error`, so a new rung is shown rather than demoted.

## The seam is data, not a trait

`painty` takes a `Diagnostic`, a borrowed view it defines itself over nothing but `core`. It does
not ask a caller to implement a trait, and it requires a diagnostic contract from no particular
crate. A renderer must not force a GraphQL library on a project that wanted an underline drawn, and
a Rust project with its own error type should not have to adopt a parser-combinator library either.
So the `tokora` feature is an adapter, one small module of conversions, and the crate is useful with
nothing enabled at all.

`painty` does not declare a `Diagnose`-shaped trait of its own and blanket-implement it for
tokora's. Two structurally identical traits in two repositories drift silently, because a trait that
has fallen behind its twin keeps compiling, where a conversion function stops building when the
contract changes. CI checks the claim rather than stating it: the consumer suites in `tests/` are
written against a hand-rolled error type and run in a build with no dependencies, and the
`no-tokora` job reads `cargo tree` for every configuration that leaves the feature off.

## Three layers

```text
Layer 1 — the description     a Diagnostic: code, severity, primary position, labels,
                              result path, help. Borrowed data, no trait, no dependency.

Layer 2 — resolution          byte offset -> (line, column); excerpt extraction;
                              the lines a region is drawn on.
                              Borrows from the source, allocation-free.
                              THE PUBLIC DATA MODEL

Layer 3 — outputs             terminal (ANSI) — four styles
                              HTML (elements and classes) — a peer of the terminal
                              SVG — the terminal render, written to a second surface
                              model export for native UIs — reserved, unimplemented
```

Resolution is a public layer rather than an implementation detail of the text renderers because a
SwiftUI or Flutter front end emits a widget tree across an FFI boundary: it cannot consume a string,
it needs the structure. Layer 2 also makes the hard part assertable structurally instead of by
snapshot. Every reported column lies within its line, an excerpt re-sliced from the source by the
reported offsets equals the span it came from, and a multi-line region opens and closes on the lines
it covers. Golden files are left covering appearance only. Bindings for SwiftUI and Flutter are
separate-repository concerns, not features of this crate.

The terminal and HTML are peers, not a renderer and a style of it. The seam the four terminal styles
are written against is denominated in display cells: one hook returns the `char` that stands in a
cell and four more position a mark by display column, and HTML has no cell. A terminal style also
marks a row from underneath, where an HTML mark is an element inside the row. What the two share is
layer 2 and the elision rule below, not the terminal's plan, which is private to its module and is
built from `Vec`s that a renderer with no allocator does not have.

Grouping labels by line and leaving out distant lines are not part of layer 2's public model. They
live in the renderers. Both draw one row per line that a span's end falls on, with every label on
that line attached to its row, and both decide which lines between two such rows are shown with one
crate-private function, so the two cannot disagree about what a reader is shown. The rule: a line
where a multi-line span opens is followed by up to three lines of context, and the rest of the gap
becomes one gap row (`...` in the `rustc` style, `⋮` in HTML). A gap that would stand for exactly
one line shows that line instead. A two-byte span across a ten-megabyte file therefore draws a
handful of rows, not millions.

## Numeric widths

A published model's integer widths cannot be changed later without breaking every consumer, and this
one is meant to cross a C ABI as well as a Rust API. So the width of a numeric member is not a local
choice: it follows from what the number *is*, by five rules read in order. The first that matches
wins, and the last matches everything, so there is no member the rules fail to place.

| # | what the number is | width | examples | why |
| - | ------------------ | ----- | -------- | --- |
| 1 | a line or column in painty's own geometry, resolved **or rendered** — including a count of lines, which is the last line's own number | `u64` | `Position::line`, `Position::column`, `Source::line_count` | it crosses a C ABI, so not `usize`; and it must not imply a ceiling the domain does not have, so not `u32` |
| 2 | an ordinal in data the **producer** built, that painty neither computes nor bounds | `u64` | `PathSegment::Index` | same ceiling argument, and there is no protocol cap to point at instead |
| 3 | a key into a structure painty does not own and never computes | that contract's width | `Location::source` (`u32`) | it has to round-trip; matching the width is what makes it lossless in both directions |
| 4 | a value painty itself **emits into a wire format** that fixes its width | that format's width | `Color::Rgb`, `Color::Ansi256` (`u8`) | the format is not negotiable and the value has to be legal in it |
| 5 | anything else — an index or a count of things in memory | `usize` | `Position::offset`, `Span::start`, `Source::len` | exactly as `slice::len` is |

The order is what resolves overlaps: a line count is both a count of things in memory and a line
ordinal, and rule 1 comes first, so it is a `u64`. And nothing on the path a position is built by
saturates or casts. A clamped ordinal is indistinguishable from a real one, which is a number that
lies rather than one that fails.

`ci/public_numeric_surface.py`, run by the `public-surface` CI job, decides. It asks rustdoc for the
resolved public API and looks for primitive integers anywhere in it, so a width behind a type alias,
a const-generic parameter or a macro expansion is found the same way as one in a return type, and
every member is recorded there with its widths. `tests/numeric_widths.rs` runs in `cargo test` and
pins each member by function pointer under the rule that placed it, so a member carrying a width its
rule does not give fails to compile, and a parameter's width is as frozen as a return's.
`tests/source_borrows.rs` pins the lifetime a borrow comes back on: everything resolution returns
borrows the source text, not the `&Source` it came through. None of these reads intent. A member
placed under a rule that gives the same width, as rules 1 and 2 both give `u64`, is caught only by
review.

## Features

Each implemented output has a feature of its own, and the default configuration selects none of
them: the crate is `no_std` and dependency-free until a caller asks for something.

| feature     | implies    | pulls in                                           | what it turns on                                                                 |
| ----------- | ---------- | -------------------------------------------------- | -------------------------------------------------------------------------------- |
| *(default)* | —          | —                                                  | layer 2: resolution, `no_std`, no dependencies                                    |
| `std`       | —          | —                                                  | anything needing the standard library                                             |
| `terminal`  | `std`      | `unicode-width`, `unicode-segmentation`, `anstyle` | the terminal renderer: cell arithmetic, colour detection, ANSI                    |
| `html`      | —          | —                                                  | the HTML renderer: escaping and CSS classes, still `no_std` and allocation-free   |
| `svg`       | `terminal` | —                                                  | `Terminal::render_svg`: the same render, as an SVG image of a terminal            |
| `model`     | —          | —                                                  | reserved; unimplemented                                                           |
| `tokora`    | —          | `tokora`                                           | `painty::tokora`: an adapter from `tokora::diagnostic::Diagnose`                  |

`terminal` takes `unicode-width` for display width and `unicode-segmentation` for UAX #29 grapheme
clusters, because owning a Unicode table means owning a class of alignment bug for no gain. Both are
pinned exactly, since the two releases carry the same Unicode data (17.0.0) that the placement tests
were surveyed against, so `terminal` constrains their versions in your dependency graph. `anstyle`
writes the SGR escape sequences.

`html` implies nothing at all: CI builds it for `thumbv6m-none-eabi`, so the renderer uses no `std`,
no `alloc` and no heap. It orders a diagnostic's labels with selection scans over the caller's own
byte offsets, which is `O(k²)` integer comparisons in the label count `k` against the terminal's
`O(k log k)`, measured at 31 ms for a thousand labels.

`svg` is the terminal render painted to a second surface, not a third renderer. The plan, the style,
the elision and the geometry are the terminal's, so every style has an SVG and its rows are
character for character the rows the terminal writes. It is an image of a terminal, a monospace
grid, because SVG has no layout engine: each grapheme cluster is drawn at an absolute coordinate
computed from the cell it was assigned, so no font metric enters the placement and a caret is under
its glyph in any face. Roles become CSS classes with `painty::html`'s names, and the document
carries a `<style>` element generated from the palette, so it stands alone while an outer stylesheet
still overrides it.

`tokora` is taken with `default-features = false`, so it stays `no_std`; `--features tokora` is one
of the bare-metal legs the `no-std` CI job builds.

### The placement model

A line is its sequence of UAX #29 extended grapheme clusters, after sanitization, and a tab expands
to its next stop (`Terminal::with_tab_width`). Each cluster occupies exactly `unicode-width`'s width
of that cluster **measured in isolation**. A byte offset's column is one plus the cells of the whole
clusters before it, and a span widens outward to cluster boundaries.

The cluster is the unit because it is the most lookahead a cursor-addressable terminal can hold
without contradicting its own cursor reports. `unicode-width` also applies rules *across* cluster
boundaries (Arabic lam followed by alef scores 1 for the pair where the clusters score 1 + 1), and
painty rejects those by specification: no terminal that reports its cursor between two characters
can implement them.

So painty promises exact cell alignment on a grapheme-aware terminal, and declines four things:

- agreement with whole-string width: six families differ on Unicode 17 data, each pinned in the
  tests at both values;
- visual alignment under bidi reordering, since columns are logical;
- font shaping: underlining half a ligature marks half the span, because those are two addressable
  source positions;
- legacy per-codepoint cell counts, where a wcwidth-era terminal gives a ZWJ emoji sequence more
  cells than its cluster width.

## Minimum supported Rust version

`1.95`, matching `tokora`, whose diagnostic contract the optional adapter reads. The `msrv` CI job
reads `rust-version` out of `Cargo.toml` rather than hardcoding a toolchain, so the declared and
tested values cannot drift. Raising the floor is not treated as a breaking change.

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
