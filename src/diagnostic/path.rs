/// One step of a path into a produced *result*, rather than into any source text.
///
/// # A coordinate in the output, not in the input
///
/// A [`Location`](super::Location) says where in a *document* something is; a path says which
/// field of which element of the *result* an error belongs to, so a client holding that result can
/// line the error up with the hole in it. The two never substitute for one another.
///
/// The motivating case is a GraphQL execution error, whose specification says the path's entries
/// "should be strings for Object fields, and 0-indexed integers for List entries" — hence exactly
/// these two variants, and hence [`Index`](Self::Index) counting from zero. The shape generalises
/// to any protocol that answers with a tree and has to say where in that tree something went
/// wrong.
///
/// # Most diagnostics carry none
///
/// Every lexical and syntactic error is about a document, so it has no result path. That is the
/// positive statement that the diagnostic cannot be associated with a particular field of a
/// result, not a missing feature.
///
/// ```
/// use painty::PathSegment;
///
/// let path = [PathSegment::Field("heroes"), PathSegment::Index(2), PathSegment::Field("name")];
/// let rendered: Vec<String> = path.iter().map(ToString::to_string).collect();
/// assert_eq!(rendered, ["heroes", "2", "name"]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathSegment<'a> {
  /// A response key — a field's alias where it has one, otherwise its name.
  Field(&'a str),
  /// A zero-based index into a list.
  Index(u32),
}

impl core::fmt::Display for PathSegment<'_> {
  #[inline]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Field(name) => f.write_str(name),
      Self::Index(index) => write!(f, "{index}"),
    }
  }
}
