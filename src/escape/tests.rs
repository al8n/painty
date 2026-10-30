use core::fmt::Write as _;

use super::{Escaped, entity_of, escaped};

/// The harness is a `std` program whatever the crate under it is, so the string these escape
/// into comes from there rather than from `alloc`, which this crate does not enable.
use std::string::String;

fn escape(text: &str) -> String {
  let mut out = String::new();
  Escaped(&mut out)
    .write_str(text)
    .expect("a `String` accepts everything");
  out
}

#[test]
fn every_escaped_character_has_an_entity_and_escaping_is_idempotent_in_meaning() {
  for character in escaped() {
    let entity = entity_of(character).expect("a character on the escape list has an entity");
    assert!(
      entity.starts_with('&') && entity.ends_with(';'),
      "{character:?} is written as {entity:?}, which is not a character reference"
    );
    // Every entity begins with `&`, so escaping one again has to escape that `&` and nothing else.
    // If it did not, an already-escaped run would round-trip and the escaper would be losing text.
    let twice = escape(entity);
    assert_eq!(
      twice,
      std::format!("&amp;{}", &entity[1..]),
      "escaping {entity:?} again did not escape its own ampersand"
    );
  }
}

#[test]
fn nothing_else_is_touched() {
  // A control character is not markup, so it is passed through here where the terminal renderer
  // substitutes a picture for it. The tab matters most: it is the one an HTML `<pre>` renders,
  // and the SVG surface never sees one because the renderer under it has already expanded or
  // substituted every control character before this writer is reached.
  let text = "a\tb\u{1b}[31mc\u{0}d\u{9b}e";
  assert_eq!(escape(text), text);
}

#[test]
fn a_run_with_nothing_to_escape_is_written_whole() {
  assert_eq!(escape(""), "");
  assert_eq!(escape("plain text"), "plain text");
}

#[test]
fn every_hostile_character_is_escaped_wherever_it_sits() {
  assert_eq!(escape("</span>&\"'<"), "&lt;/span&gt;&amp;&quot;&#39;&lt;");
  assert_eq!(escape("&lt;"), "&amp;lt;");
}
