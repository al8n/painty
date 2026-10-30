//! The one thing between a caller's text and a markup document that would otherwise execute it.
//!
//! # Why this is not the HTML renderer's
//!
//! It was, until there was a second markup output. The rule below is the same five characters for
//! HTML and for the XML an SVG document is, so a copy behind the second feature would be the shape
//! this crate has already paid for twice — `Input`, and the elision rule, the second of which
//! produced a divergence before it was one module. An escaping census is worth having only if it
//! is a question about one file.
//!
//! # Why an adapter and not a function over `&str`
//!
//! [`Diagnostic::message`](crate::Diagnostic::message) is a `&dyn Display`, so a message's own
//! `Display` impl writes into the output directly. A function taking `&str` cannot see that text —
//! it would have to be collected first, which this crate has no allocator for — so a writer that
//! substitutes as it goes is the only shape that covers both. It is the same shape the terminal
//! renderer's control-character sanitizer takes, and for the same reason.
//!
//! # Five characters, not three
//!
//! `&`, `<` and `>` are what element content needs. `"` and `'` are what an attribute value needs,
//! and they are escaped here rather than at the sites that write one, because "which context is
//! this?" is a question a writer three calls down the stack cannot answer and a reviewer cannot
//! check. One rule, applied everywhere, is a rule an escaping census can state.
//!
//! Nothing else is touched, and the two consumers are owed different accounts of why. A control
//! character is not an injection vector in **HTML** — it is not markup, it cannot leave its
//! element, and a browser renders it as nothing — where in a terminal `\x1b` *is* the escape
//! mechanism.
//!
//! **XML asks a second question, and it is not this file's.** A scalar outside XML 1.0's `Char`
//! production is not representable at all — not even as a numeric reference — so a document
//! carrying one does not parse. That set is *larger* than the control characters, and reading it as
//! "the C0 characters" is how it was got wrong here once: the terminal renderer's substitution does
//! sit under the SVG surface and does keep every C0 out of it, and U+FFFE and U+FFFF walked past it
//! untouched, because a sanitizer built on Control Pictures has no picture for a noncharacter. So
//! the whole production is enumerated at the boundary that needs it, by
//! `terminal::svg::Representable`, and this file stays the answer to "what would stop being text".

#[cfg(test)]
mod tests;

use core::fmt;

/// A writer that escapes as it goes.
pub(crate) struct Escaped<'a>(pub(crate) &'a mut dyn fmt::Write);

impl fmt::Write for Escaped<'_> {
  /// Written in runs rather than character by character, so ordinary text costs one `write_str`
  /// and only the escaped characters cost a call of their own.
  fn write_str(&mut self, text: &str) -> fmt::Result {
    let mut rest = text;
    while let Some(at) = rest.find(ESCAPED) {
      let (before, from) = rest.split_at(at);
      self.0.write_str(before)?;
      let mut characters = from.chars();
      let escaped = characters
        .next()
        .expect("`find` reported a character at this index");
      self.0.write_str(
        entity(escaped).expect("`find` matches only the characters `entity` has an entity for"),
      )?;
      rest = characters.as_str();
    }
    self.0.write_str(rest)
  }
}

/// Every character that leaves text and becomes markup.
const ESCAPED: [char; 5] = ['&', '<', '>', '"', '\''];

/// What one of [`ESCAPED`] is written as, and `None` for anything else.
///
/// `&#39;` rather than `&apos;`: the named form is XML's and HTML 4 did not have it, so a numeric
/// reference is the one every parser has always understood.
const fn entity(character: char) -> Option<&'static str> {
  Some(match character {
    '&' => "&amp;",
    '<' => "&lt;",
    '>' => "&gt;",
    '"' => "&quot;",
    '\'' => "&#39;",
    _ => return None,
  })
}

/// The escape list, for the test that holds it and the `match` above to the same set.
///
/// The two are declared apart — one is a `find` pattern, the other a `match` — and the `expect` in
/// `write_str` is what a disagreement between them would reach. `every_escaped_character_has_an_
/// entity` is what makes that a failing test rather than a panic in a caller's render.
#[cfg(test)]
pub(crate) const fn escaped() -> [char; 5] {
  ESCAPED
}

/// The entity table, for the same test.
#[cfg(test)]
pub(crate) const fn entity_of(character: char) -> Option<&'static str> {
  entity(character)
}
