# Changelog

## 0.0.0 - 2026-10-30

First release of `painty`.

- Resolves diagnostic locations into source lines, columns, excerpts, and spans.
- Renders diagnostics in four terminal styles: `rustc`, `miette`, `ariadne`, and `codespan`.
- Renders diagnostics as HTML and terminal output as SVG.
- Sanitizes caller-supplied control characters before terminal output.
- Provides an optional adapter for `tokora` diagnostics.
