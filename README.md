<div align="center">
<h1>painty</h1>
</div>
<div align="center">

Renders a diagnostic and its source text to a terminal, to HTML, or to an exportable model.

[![Build](https://img.shields.io/github/actions/workflow/status/al8n/painty/ci.yml?logo=Github-Actions&style=for-the-badge)](https://github.com/al8n/painty/actions/workflows/ci.yml)
[![github](https://img.shields.io/badge/github-al8n/painty-8da0cb?style=for-the-badge&logo=Github)](https://github.com/al8n/painty)
[![license](https://img.shields.io/badge/License-Apache%202.0%2FMIT-blue.svg?style=for-the-badge)](https://github.com/al8n/painty#license)

</div>

## Status

**Skeleton — there is no renderer yet.** This repository currently carries the crate's identity,
its feature surface and its CI gates, and nothing else. That order is deliberate: the gates go in
while an empty crate cannot make them fail, so the first line of rendering code is measured
against something rather than retrofitted with a set of checks chosen to suit it.

Nothing here is published to crates.io.

## Overview

A diagnostic contract is worth more when it does not know how it will be displayed. Producers —
[smear](https://github.com/al8n/smear), [pql](https://github.com/al8n/pql), and
[tokora](https://github.com/al8n/tokora)'s coming SQL, Yul and Solidity frontends — describe what
went wrong through a borrowing, allocation-free, `no_std`-reachable trait that mentions no colours,
no terminal and no markup. `painty` is the other half of that split: give it such a value and the
source text it points into, and it produces something a human, a browser or a native UI can read.

Existing renderers do not fit that shape. `miette` and `ariadne` both require `std`, both own their
strings, and both bake a rendering strategy into the error type — which is the coupling the
contract exists to avoid.

## Three layers

The layer boundary is where the design does its real work, and it is not the obvious one. A
terminal renderer and an HTML renderer both emit text, so their shared work could live inside
either of them. A SwiftUI or Flutter front end emits a widget tree, on the far side of an FFI
boundary, in another language — it cannot consume a string, it needs the structure. So resolution
becomes a public layer of its own rather than an implementation detail of the text renderers:

```text
Layer 1 — the contract        the diagnostic trait, in tokora
                              zero-allocation, no_std, no dependencies

Layer 2 — resolution          byte offset -> (line, column); excerpt extraction; grouping
                              labels by line; ordering; elision; overlap resolution
                              borrows from the source, still allocation-free
                              THE PUBLIC DATA MODEL

Layer 3 — outputs             terminal (ANSI) - HTML - model export for native UIs
```

Layer 2 earns its keep twice. Beyond serving outputs that are not text, it is what makes the hard
part assertable structurally instead of by snapshot: every reported column lies within its line's
expanded width, no two label placements occupy one cell, an excerpt re-sliced from the source by
the reported offsets equals the span it came from. Golden files are then left covering only
appearance, where re-blessing one is cheap and correct.

## Features

Every output is independently selectable, and the default configuration selects none of them — the
crate is `no_std` and dependency-free until a caller asks for a renderer.

| feature    | implies | pulls in                  | what it turns on                                    |
| ---------- | ------- | ------------------------- | --------------------------------------------------- |
| *(default)* | —      | —                         | layer 2 only: resolution, `no_std`, no dependencies |
| `std`      | —       | —                         | anything needing the standard library               |
| `terminal` | `std`   | `unicode-width`, `anstyle` | ANSI output, box drawing, display-width alignment   |
| `html`     | —       | —                         | HTML output: escaping and CSS classes               |
| `model`    | —       | —                         | a stable C-ABI export of the resolved model         |

```toml
[dependencies]
painty = { version = "0", features = ["terminal"] }
```

`unicode-width` is taken rather than hand-narrowed because correct terminal alignment is impossible
without display width, and owning a Unicode table means owning a class of alignment bug for no
gain. `anstyle` is what `clap` and `cargo` already use, so a consumer's `--color` flag and this
crate's theme interoperate without a conversion layer. Both sit behind `terminal`; the HTML and
model outputs pull in neither.

SwiftUI and Flutter bindings are deliberately not features. They are separate repositories
consuming the `model` export, so this crate never needs to know Swift or Dart exist.

## Minimum supported Rust version

`1.95`, matching `tokora` — the crate whose diagnostic contract `painty` renders. A renderer with a
lower floor than its contract would be advertising a floor no caller could actually build on. The
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
