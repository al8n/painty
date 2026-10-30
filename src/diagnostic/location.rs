use core::cmp::Ordering;

use super::Span;

/// A position in one of the inputs a diagnostic was produced from.
///
/// `source` names which input, as an index into whatever list the caller was given; a single-input
/// caller answers `0`. Mapping that index to a filename, and to the text itself, stays with the
/// caller — painty renders source text it is handed and owns no notion of a file.
///
/// # `entire` is the honest answer for an input with no positions
///
/// A machine-generated document, a binary blob, a request synthesized from configuration: a byte
/// offset into one of those points at nothing anybody can edit. Such a producer answers
/// [`Location::entire`], whose [`span`](Location::span) is `None`, and a renderer is told the
/// diagnostic is about the input as a whole rather than being handed a fabricated `0..len`.
///
/// ```
/// use painty::{Location, Span};
///
/// let at = Location::new(0, Span::new(12, 19));
/// assert_eq!(at.source(), 0);
/// assert_eq!(at.span(), Some(Span::new(12, 19)));
/// assert!(!at.is_entire());
///
/// let whole = Location::entire(1);
/// assert_eq!(whole.span(), None);
/// assert!(whole.is_entire());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
  source: u32,
  span: Option<Span>,
}

impl Location {
  /// A range within `source`.
  #[inline]
  pub const fn new(source: u32, span: Span) -> Self {
    Self {
      source,
      span: Some(span),
    }
  }

  /// The whole of `source`, for an input with no positions to point at.
  #[inline]
  pub const fn entire(source: u32) -> Self {
    Self { source, span: None }
  }

  /// Returns which input the location belongs to.
  #[inline]
  pub const fn source(&self) -> u32 {
    self.source
  }

  /// Returns the byte range, or `None` when the location is the whole input.
  #[inline]
  pub const fn span(&self) -> Option<Span> {
    self.span
  }

  /// Returns whether the location is the whole input rather than a range within it.
  #[inline]
  pub const fn is_entire(&self) -> bool {
    self.span.is_none()
  }
}

/// A secondary position with a phrase explaining what is there.
///
/// "declared here", "first defined here", "the other selection" — the second half of a diagnostic
/// that is about a *relationship* between two places.
///
/// The text borrows rather than being `&'static str`. A rule's fixed phrase satisfies that
/// trivially, and a caller whose phrase is computed — interpolating a name it already holds in a
/// buffer — is not shut out of the only field where that is likely to matter.
///
/// ```
/// use painty::{Label, Location, Span};
///
/// let label = Label::new(Location::new(0, Span::new(3, 7)), "first defined here");
/// assert_eq!(label.text(), "first defined here");
/// assert_eq!(label.location().span(), Some(Span::new(3, 7)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label<'a> {
  location: Location,
  text: &'a str,
}

impl<'a> Label<'a> {
  /// Attaches `text` to `location`.
  #[inline]
  pub const fn new(location: Location, text: &'a str) -> Self {
    Self { location, text }
  }

  /// Returns where the label points.
  #[inline]
  pub const fn location(&self) -> Location {
    self.location
  }

  /// Returns what the label says.
  #[inline]
  pub const fn text(&self) -> &'a str {
    self.text
  }

  /// Orders labels in place for a single forward walk over the source.
  ///
  /// # What the order is
  ///
  /// By input first, then by where the label starts, then — for two labels starting together — by
  /// the one that *ends later* first, so an enclosing range precedes the ranges nested inside it.
  /// A label with no span at all ([`Location::entire`]) sorts before every positioned label in its
  /// input, because it is about that input as a whole.
  ///
  /// # Why a caller would want it
  ///
  /// Resolution walks the text forwards, and a walk that has to jump backwards has to start over:
  /// every offset resolved out of order costs another scan from the top. Ordering once makes the
  /// whole set resolvable in one pass. It sorts a slice the caller already owns, so it allocates
  /// nothing — which is the reason this is a function over a slice rather than something that
  /// hands back a sorted collection.
  ///
  /// # It is a total order on what is visible
  ///
  /// The sort is unstable, so ties would otherwise be broken arbitrarily and two runs could
  /// disagree. The final key is the label's text, which makes the comparison total over
  /// everything a reader can see: two labels that compare equal here are indistinguishable in any
  /// output, so the result is deterministic without paying for a stable sort.
  ///
  /// ```
  /// use painty::{Label, Location, Span};
  ///
  /// let mut labels = [
  ///   Label::new(Location::new(0, Span::new(20, 24)), "third"),
  ///   Label::new(Location::new(0, Span::new(4, 12)), "second, and it encloses"),
  ///   Label::new(Location::new(0, Span::new(4, 8)), "the one nested inside it"),
  ///   Label::new(Location::entire(0), "first — no position of its own"),
  /// ];
  /// Label::order(&mut labels);
  ///
  /// let texts: Vec<&str> = labels.iter().map(|label| label.text()).collect();
  /// assert_eq!(texts, [
  ///   "first — no position of its own",
  ///   "second, and it encloses",
  ///   "the one nested inside it",
  ///   "third",
  /// ]);
  /// ```
  pub fn order(labels: &mut [Self]) {
    labels.sort_unstable_by(|a, b| {
      a.location
        .source()
        .cmp(&b.location.source())
        .then_with(|| match (a.location.span(), b.location.span()) {
          (None, None) => Ordering::Equal,
          (None, Some(_)) => Ordering::Less,
          (Some(_), None) => Ordering::Greater,
          (Some(left), Some(right)) => left
            .start()
            .cmp(&right.start())
            .then_with(|| right.end().cmp(&left.end())),
        })
        .then_with(|| a.text.cmp(b.text))
    });
  }
}
