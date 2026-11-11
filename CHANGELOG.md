# Changelog

## Unreleased

- The `tokora` adapter reads tokora 0.13, whose label phrases, primary label and help line are
  borrowed from the diagnostic: `From<tokora::diagnostic::Label<'a>>` for `Label<'a>` replaces
  `From<tokora::diagnostic::Label>` for `Label<'static>`, and a phrase a diagnostic composed,
  such as "expected `HashSet<String>`", reaches the view like a literal one. Breaking for a
  caller with the `tokora` feature on, whose errors must implement tokora 0.13's `Diagnose`.

## 0.0.0 - 2026-10-30

First release of `painty`.

- Resolves diagnostic locations into source lines, columns, excerpts, and spans.
- Renders diagnostics in four terminal styles: `rustc`, `miette`, `ariadne`, and `codespan`.
- Renders diagnostics as HTML and terminal output as SVG.
- Sanitizes caller-supplied control characters before terminal output.
- Provides an optional adapter for `tokora` diagnostics.
