/// How much a diagnostic asks of its reader.
///
/// # Three rungs, and a fourth that has to be survivable
///
/// `#[non_exhaustive]`, so a consumer matching on it carries a fallback arm and a fourth rung is
/// an addition rather than a break. That openness costs painty's own renderers nothing:
/// `#[non_exhaustive]` binds other crates, so a match written inside this crate stays exhaustive
/// and a rung added here fails to compile until every renderer has decided what to do with it.
///
/// # Why painty declares its own rather than borrowing one
///
/// This is the same ladder `tokora`'s diagnostic contract publishes, and the duplication is
/// deliberate: a crate whose whole job is to draw an underline must not require its caller to
/// adopt a parser-combinator library first. The `tokora` feature supplies a [`From`] between the
/// two, and a conversion is a function — when the two ladders diverge it stops compiling, which is
/// the failure direction a second *trait* would not have given.
///
/// ```
/// use painty::Severity;
///
/// assert_eq!(Severity::Error.as_str(), "error");
/// assert_eq!(Severity::Warning.as_str(), "warning");
/// assert_eq!(Severity::Advice.as_str(), "advice");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Severity {
  /// The input is refused. A verdict depends on it.
  Error,
  /// The input is accepted and something about it is worth saying.
  Warning,
  /// A suggestion, carrying no judgement about the input.
  Advice,
}

impl Severity {
  /// Returns the rung's lowercase name.
  #[inline]
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Error => "error",
      Self::Warning => "warning",
      Self::Advice => "advice",
    }
  }
}

impl core::fmt::Display for Severity {
  #[inline]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str(self.as_str())
  }
}
