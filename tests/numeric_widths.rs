//! Every public numeric is the width its rule gives it, and no position is ever clamped.
//!
//! # Why these are assertions and not scenarios
//!
//! The behaviour worth guaranteeing is that painty never reports a number that has been quietly
//! narrowed, and never publishes a width it cannot change later. The scenario that would exercise
//! a clamp needs a source with more than `u32::MAX` line breaks — over four gigabytes of text — so
//! no test is going to construct it, and a guarantee with no test behind it decays into a comment.
//! So the *property* is asserted instead.
//!
//! # The census asks an authority; it used to recognise a pattern
//!
//! Everything here that reads painty's source parses it with [`syn`]. The first two versions
//! scanned lines for `pub fn`, and that recogniser was wrong five times across two reviews: it
//! missed a rustfmt-wrapped signature, a trait method, a public field, a name that is a prefix of
//! a pinned one, and a duplicated exemption. Each was patched and the next appeared, which is what
//! a checker built on a pattern does — the pattern and the reader come from the same head at the
//! same moment, so one worked example proves almost nothing about its precision. A parser has no
//! such blind spot: it either accepts Rust or it does not.
//!
//! # The four layers
//!
//! **Every signature, by function pointer.** [`pin`] ascribes the whole signature of every public
//! member that mentions a primitive integer. A parameter's width is as frozen as a return's, and
//! the receiver's borrow is written `for<'s>` rather than tied to the source lifetime — see
//! [`pin`] for why that distinction is load-bearing rather than pedantic.
//!
//! **That the list is complete, and no longer than the surface.** The public members are read out
//! of `src/` and matched against the ascriptions by *exact* path, in both directions.
//!
//! **`u32`, by exact member.** Rule 3 in `README.md` is the only rule yielding a narrow type, and
//! it justifies exactly four occurrences. The exemption names those four members and requires each
//! to match exactly once, so a duplicate cannot shelter under an existing entry and an entry
//! cannot outlive what it licensed.
//!
//! **The arithmetic.** No saturating call and no `as` cast on the path a position is built by.
//!
//! # What still gets through
//!
//! 1. **A member placed under the wrong rule.** A new `usize` that should have been a rule-2
//!    ordinal is pinned, complete, `u32`-free — and wrong. A parser does not read intent, so this
//!    is untouched by everything above and is the residual that matters. It is also the only one
//!    that has happened: twice, and review caught it both times.
//! 2. **A width painty does not choose.** A method of `impl Iterator for RegionLines` takes its
//!    signature from the trait, and `src/tokora.rs`'s conversions take theirs from tokora. Those
//!    are excluded from the *completeness* census on purpose — painty cannot pick them — but they
//!    are inside the `u32` census, so a narrow type still has to be argued for.
//! 3. **A public numeric reached through a re-exported foreign type.** `src/lib.rs` re-exports
//!    only painty's own types today, so there is nothing to reach; a future `pub use` of somebody
//!    else's type would carry its widths past every check here, because the parser is pointed at
//!    painty's files rather than at a resolved API graph.
//!
//! Item 1 needs a reader. Item 3 needs the compiler's own view of the public surface rather than a
//! parse of the crate's text, which is a different and much larger machine than this one.
//!
//! # What "exact" rests on
//!
//! A line ordinal is at most one more than the text's length in bytes; Rust caps a single object
//! at `isize::MAX` bytes; so on the widest target this crate builds for, an ordinal cannot exceed
//! 2^63 and a `u64` cannot overflow. The increments are therefore plain `+`, and a debug build
//! panics if that reasoning is ever wrong — which is the direction to fail in. A saturating
//! increment would instead hand back a number indistinguishable from a real one.

use painty::{Line, LineBreak, Location, PathSegment, Position, Region, RegionLine, Source, Span};
use syn::{
  Item, Type, Visibility,
  spanned::Spanned,
  visit::{self, Visit},
};

/// Every file that declares part of the public surface.
///
/// A literal list, because `include_str!` needs one — and a literal list goes stale in silence, so
/// [`the_file_list_is_the_whole_crate`] walks `src/` and checks it. `src/tokora.rs` is here whether
/// or not its feature is on: `include_str!` reads the disk, not the build, which is what keeps the
/// adapter under the same censuses as everything else.
const CRATE: [(&str, &str); 10] = [
  ("src/lib.rs", include_str!("../src/lib.rs")),
  (
    "src/diagnostic/mod.rs",
    include_str!("../src/diagnostic/mod.rs"),
  ),
  (
    "src/diagnostic/location.rs",
    include_str!("../src/diagnostic/location.rs"),
  ),
  (
    "src/diagnostic/path.rs",
    include_str!("../src/diagnostic/path.rs"),
  ),
  (
    "src/diagnostic/severity.rs",
    include_str!("../src/diagnostic/severity.rs"),
  ),
  (
    "src/diagnostic/span.rs",
    include_str!("../src/diagnostic/span.rs"),
  ),
  ("src/source/mod.rs", include_str!("../src/source/mod.rs")),
  ("src/source/line.rs", include_str!("../src/source/line.rs")),
  (
    "src/source/region.rs",
    include_str!("../src/source/region.rs"),
  ),
  ("src/tokora.rs", include_str!("../src/tokora.rs")),
];

/// The files a position is built by.
const RESOLUTION: [&str; 3] = [
  "src/source/mod.rs",
  "src/source/line.rs",
  "src/source/region.rs",
];

/// This file, so the completeness census can read [`pin`]'s ascriptions out of it.
const SELF: &str = include_str!("numeric_widths.rs");

/// The primitive integer types.
const INTEGERS: [&str; 12] = [
  "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// The four occurrences of `u32` rule 3 licenses, named by the member each belongs to.
///
/// A member rather than a line of text. Keyed on text, two identical `source: u32,` fields would
/// both match one entry and the second would never be reported; keyed on the member, they cannot
/// collide, and the count below makes each entry single-use in both directions.
const RULE_THREE: [(&str, &str); 4] = [
  ("src/diagnostic/location.rs", "Location::source"),
  ("src/diagnostic/location.rs", "Location::source()"),
  ("src/diagnostic/location.rs", "Location::new()"),
  ("src/diagnostic/location.rs", "Location::entire()"),
];

/// A public member painty chooses a width for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Member {
  path: String,
  site: String,
}

/// Somewhere a rule or a discipline could be broken, named by the member that owns it.
#[derive(Debug, Clone)]
struct Site {
  owner: String,
  site: String,
}

#[derive(Default)]
struct Surface {
  members: Vec<Member>,
  narrows: Vec<Site>,
  saturating: Vec<Site>,
  casts: Vec<Site>,
}

fn type_name(ty: &Type) -> String {
  match ty {
    Type::Path(path) => path
      .path
      .segments
      .last()
      .map_or_else(String::new, |segment| segment.ident.to_string()),
    Type::Reference(reference) => type_name(&reference.elem),
    _ => String::new(),
  }
}

fn is_public(visibility: &Visibility) -> bool {
  matches!(visibility, Visibility::Public(_))
}

/// Collects the primitive integers named anywhere inside one syntax node.
#[derive(Default)]
struct Integers(Vec<String>);

impl<'ast> Visit<'ast> for Integers {
  fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
    if node.qself.is_none()
      && let Some(ident) = node.path.get_ident()
      && INTEGERS.contains(&ident.to_string().as_str())
    {
      self.0.push(ident.to_string());
    }
    visit::visit_type_path(self, node);
  }
}

fn integers_in_signature(signature: &syn::Signature) -> Vec<String> {
  let mut found = Integers::default();
  found.visit_signature(signature);
  found.0
}

fn integers_in_type(ty: &Type) -> Vec<String> {
  let mut found = Integers::default();
  found.visit_type(ty);
  found.0
}

/// Reads one file's declarations, tracking which member each syntax node belongs to.
struct Scan<'a> {
  file: &'a str,
  resolution: bool,
  owner: String,
  inherent: bool,
  public_container: bool,
  out: &'a mut Surface,
}

impl Scan<'_> {
  fn site(&self, span: proc_macro2::Span) -> String {
    format!("{}:{}", self.file, span.start().line)
  }

  fn member(&mut self, path: String, span: proc_macro2::Span) {
    let site = self.site(span);
    self.out.members.push(Member { path, site });
  }

  /// Runs `body` with `owner` in scope, then restores the previous one.
  fn scoped(&mut self, owner: String, body: impl FnOnce(&mut Self)) {
    let previous = core::mem::replace(&mut self.owner, owner);
    body(self);
    self.owner = previous;
  }
}

impl<'ast> Visit<'ast> for Scan<'_> {
  fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
    let inherent = core::mem::replace(&mut self.inherent, node.trait_.is_none());
    self.scoped(type_name(&node.self_ty), |scan| {
      for item in &node.items {
        scan.visit_impl_item(item);
      }
    });
    self.inherent = inherent;
  }

  fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
    let path = format!("{}::{}", self.owner, node.sig.ident);
    // A trait impl's widths are the trait's, not painty's, so it is not a member painty places —
    // but it is still scanned, because a narrow type inside one is still a narrow type.
    if self.inherent && is_public(&node.vis) && !integers_in_signature(&node.sig).is_empty() {
      self.member(path.clone(), node.sig.ident.span());
    }
    self.scoped(format!("{path}()"), |scan| {
      visit::visit_impl_item_fn(scan, node);
    });
  }

  fn visit_impl_item_const(&mut self, node: &'ast syn::ImplItemConst) {
    let path = format!("{}::{}", self.owner, node.ident);
    if self.inherent && is_public(&node.vis) && !integers_in_type(&node.ty).is_empty() {
      self.member(path.clone(), node.ident.span());
    }
    self.scoped(path, |scan| visit::visit_impl_item_const(scan, node));
  }

  fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
    let public = core::mem::replace(&mut self.public_container, is_public(&node.vis));
    self.scoped(node.ident.to_string(), |scan| {
      visit::visit_item_struct(scan, node);
    });
    self.public_container = public;
  }

  fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
    let public = core::mem::replace(&mut self.public_container, is_public(&node.vis));
    self.scoped(node.ident.to_string(), |scan| {
      visit::visit_item_enum(scan, node);
    });
    self.public_container = public;
  }

  fn visit_variant(&mut self, node: &'ast syn::Variant) {
    let path = format!("{}::{}", self.owner, node.ident);
    // A variant's fields are as public as its enum, so there is no per-field visibility to read.
    let numeric = node
      .fields
      .iter()
      .any(|field| !integers_in_type(&field.ty).is_empty());
    if self.public_container && numeric {
      self.member(path.clone(), node.ident.span());
    }
    self.scoped(path, |scan| visit::visit_variant(scan, node));
  }

  fn visit_field(&mut self, node: &'ast syn::Field) {
    // Named fields extend the owner; a tuple field keeps its variant's or struct's name, which is
    // what a caller writes anyway.
    let path = match &node.ident {
      Some(ident) => format!("{}::{}", self.owner, ident),
      None => self.owner.clone(),
    };
    if self.public_container && is_public(&node.vis) && !integers_in_type(&node.ty).is_empty() {
      self.member(path.clone(), node.ty.span());
    }
    self.scoped(path, |scan| visit::visit_field(scan, node));
  }

  fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
    let public = core::mem::replace(&mut self.public_container, is_public(&node.vis));
    self.scoped(node.ident.to_string(), |scan| {
      visit::visit_item_trait(scan, node);
    });
    self.public_container = public;
  }

  fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
    let path = format!("{}::{}", self.owner, node.sig.ident);
    if self.public_container && !integers_in_signature(&node.sig).is_empty() {
      self.member(path.clone(), node.sig.ident.span());
    }
    self.scoped(format!("{path}()"), |scan| {
      visit::visit_trait_item_fn(scan, node);
    });
  }

  fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
    let path = node.sig.ident.to_string();
    if is_public(&node.vis) && !integers_in_signature(&node.sig).is_empty() {
      self.member(path.clone(), node.sig.ident.span());
    }
    self.scoped(format!("{path}()"), |scan| {
      visit::visit_item_fn(scan, node);
    });
  }

  fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
    let path = node.ident.to_string();
    if is_public(&node.vis) && !integers_in_type(&node.ty).is_empty() {
      self.member(path.clone(), node.ident.span());
    }
    self.scoped(path, |scan| visit::visit_item_const(scan, node));
  }

  fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
    let path = node.ident.to_string();
    if is_public(&node.vis) && !integers_in_type(&node.ty).is_empty() {
      self.member(path.clone(), node.ident.span());
    }
    self.scoped(path, |scan| visit::visit_item_type(scan, node));
  }

  fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
    if node.qself.is_none()
      && let Some(ident) = node.path.get_ident()
      && ident == "u32"
    {
      let site = self.site(ident.span());
      self.out.narrows.push(Site {
        owner: self.owner.clone(),
        site,
      });
    }
    visit::visit_type_path(self, node);
  }

  fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
    if self.resolution && node.method.to_string().starts_with("saturating_") {
      let site = self.site(node.method.span());
      self.out.saturating.push(Site {
        owner: self.owner.clone(),
        site,
      });
    }
    visit::visit_expr_method_call(self, node);
  }

  fn visit_expr_cast(&mut self, node: &'ast syn::ExprCast) {
    if self.resolution && !integers_in_type(&node.ty).is_empty() {
      let site = self.site(node.as_token.span());
      self.out.casts.push(Site {
        owner: self.owner.clone(),
        site,
      });
    }
    visit::visit_expr_cast(self, node);
  }
}

/// Parses every file in [`CRATE`] and reads its numeric surface.
fn surface() -> Surface {
  let mut out = Surface::default();
  for (file, source) in CRATE {
    let parsed =
      syn::parse_file(source).unwrap_or_else(|error| panic!("{file} does not parse: {error}"));
    let mut scan = Scan {
      file,
      resolution: RESOLUTION.contains(&file),
      owner: "<file>".to_owned(),
      inherent: true,
      public_container: false,
      out: &mut out,
    };
    for item in &parsed.items {
      scan.visit_item(item);
    }
  }
  out
}

/// The paths [`pin`] ascribes, read out of this file's own syntax rather than its text.
fn ascribed() -> Vec<String> {
  #[derive(Default)]
  struct Locals(Vec<String>);
  impl<'ast> Visit<'ast> for Locals {
    fn visit_local(&mut self, node: &'ast syn::Local) {
      if let Some(init) = &node.init
        && let syn::Expr::Path(path) = &*init.expr
      {
        let joined = path
          .path
          .segments
          .iter()
          .map(|segment| segment.ident.to_string())
          .collect::<Vec<_>>()
          .join("::");
        self.0.push(joined);
      }
      visit::visit_local(self, node);
    }
  }

  let parsed = syn::parse_file(SELF).expect("this test file parses");
  let mut locals = Locals::default();
  for item in &parsed.items {
    if let Item::Fn(function) = item
      && function.sig.ident == "pin"
    {
      locals.visit_item_fn(function);
    }
  }
  locals.0
}

/// The whole public numeric surface, one ascription per member.
///
/// # Why a function pointer
///
/// It fixes the parameters and the return in one line and cannot be written while forgetting an
/// argument. A parameter's width is as frozen as a return's: `Span::new` taking `usize` is a
/// promise to every caller who has already written one.
///
/// # Why the receiver's borrow is `for<'s>` and not `'a`
///
/// This is subtler than it looks, and getting it wrong left a real hole. Writing
/// `fn(&'a Source<'a>, u64) -> Option<Line<'a>>` looks stricter than it is: it lets the receiver
/// borrow *absorb* the source lifetime, so a regression to `-> Option<Line<'_>>` — returning a line
/// tied to the temporary `&self` borrow instead of to the text — instantiates `'s := 'a` and
/// satisfies the pin. Verified by planting exactly that and watching the old form accept it.
///
/// The receiver is therefore written `&Source<'a>`, with the borrow **elided**. An elided lifetime in
/// a function-pointer type is higher-ranked, so this reads "for every receiver borrow, returning
/// `'a`" — which the regression cannot satisfy. Writing `&'a Source<'a>` here looks stricter and is
/// the hole; the fully higher-ranked `for<'a, 's>` is the opposite error, more general than the
/// method and refused outright. Verified by planting the regression against all three.
///
/// The crate denies `single_use_lifetimes`, so the explicit `for<'s>` spelling is not available
/// anyway — which is one reason `tests/source_borrows.rs` asserts the same property from the other
/// side, by keeping the returned value alive after the `Source` is gone. That check does not
/// depend on anybody getting a function-pointer type right.
///
/// The witness gives `'a` somewhere to come from; a lifetime used only inside the body would read
/// to clippy as an unused one.
fn pin<'a>(_witness: &'a ()) {
  // Rule 1 — a line or column in the resolved model.
  let _: fn(&Position) -> u64 = Position::line;
  let _: fn(&Position) -> u64 = Position::column;
  let _: fn(&Line<'a>) -> u64 = Line::number;
  let _: fn(&Line<'a>) -> u64 = Line::char_count;
  let _: fn(&Line<'a>, usize) -> u64 = Line::column_at;
  let _: fn(&Source<'a>) -> u64 = Source::line_count;
  let _: fn(&Source<'a>, u64) -> Option<Line<'a>> = Source::line;
  let _: fn(&Region<'a>) -> u64 = Region::line_count;
  let _: fn(&RegionLine<'a>) -> core::ops::Range<u64> = RegionLine::columns;

  // Rule 2 — an ordinal in data the producer built.
  let _: fn(u64) -> PathSegment<'a> = PathSegment::Index;

  // Rule 3 — a key into a structure painty does not own.
  let _: fn(u32, Span) -> Location = Location::new;
  let _: fn(u32) -> Location = Location::entire;
  let _: fn(&Location) -> u32 = Location::source;

  // Rule 4 — everything else: an index or a count of things in memory.
  let _: fn(usize, usize) -> Span = Span::new;
  let _: fn(usize) -> Span = Span::empty;
  let _: fn(&Span) -> usize = Span::start;
  let _: fn(&Span) -> usize = Span::end;
  let _: fn(&Span) -> usize = Span::len;
  let _: fn(&Span, usize) -> bool = Span::contains;
  let _: fn(&Position) -> usize = Position::offset;
  let _: fn(&LineBreak) -> usize = LineBreak::byte_len;
  let _: fn(&Source<'a>) -> usize = Source::len;
  let _: fn(&Source<'a>, usize) -> Line<'a> = Source::line_at;
  let _: fn(&Source<'a>, usize) -> Position = Source::position;

  #[cfg(feature = "tokora")]
  {
    use painty::tokora::Adapted;
    let _: fn(&Adapted<'a>) -> usize = Adapted::dropped_labels;
    let _: fn(&Adapted<'a>) -> usize = Adapted::dropped_path_segments;
  }
}

#[test]
fn the_file_list_is_the_whole_crate() {
  // Every census here reads `CRATE`, so a source file missing from it is a file no census covers —
  // and nothing about a passing run would say so. Walked at run time and compared, rather than
  // trusted: the literal is what `include_str!` requires, and this is what keeps the literal true.
  fn walk(directory: &std::path::Path, found: &mut Vec<String>, root: &std::path::Path) {
    for entry in std::fs::read_dir(directory).expect("src/ is readable") {
      let path = entry.expect("a readable directory entry").path();
      if path.is_dir() {
        walk(&path, found, root);
      } else if path.extension().is_some_and(|extension| extension == "rs") {
        let relative = path
          .strip_prefix(root)
          .expect("a path under the crate root");
        found.push(relative.to_string_lossy().replace('\\', "/"));
      }
    }
  }

  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut on_disk = Vec::new();
  walk(&root.join("src"), &mut on_disk, root);
  on_disk.sort();

  // `tests.rs` files are unit tests rather than declarations, and are the one thing the list omits.
  let mut expected: Vec<String> = CRATE.iter().map(|(path, _)| (*path).to_owned()).collect();
  expected.extend(
    on_disk
      .iter()
      .filter(|path| path.ends_with("/tests.rs"))
      .cloned(),
  );
  expected.sort();

  assert_eq!(
    on_disk, expected,
    "`src/` and the census file list disagree. A new source file is covered by no check here until \
     it is added to CRATE; a deleted one leaves an entry that reads an empty file."
  );
}

#[test]
fn every_public_numeric_member_is_pinned() {
  pin(&());

  let ascribed = ascribed();
  let missing: Vec<_> = surface()
    .members
    .into_iter()
    .filter(|member| !ascribed.contains(&member.path))
    .map(|member| format!("{}: {}", member.site, member.path))
    .collect();

  assert!(
    missing.is_empty(),
    "a public member declares a primitive integer and nothing pins it. Place it with the ordered \
     rule in README.md#numeric-widths, then add a `let _: fn(..) -> ..` line for it to `pin`:\n{}",
    missing.join("\n")
  );
}

#[test]
fn no_ascription_outlives_the_member_it_pins() {
  // The other direction, so the list can only be as long as the surface. Without it a removed or
  // renamed member leaves an ascription that still compiles against nothing anybody calls, and the
  // census above keeps passing over a list that has quietly stopped describing the crate.
  //
  // Matched by EXACT path. `contains` would let `Region::line_count` stand in as the pin for a new
  // `Region::line`, and both censuses would stay green over an unpinned member.
  let members = surface().members;
  let stale: Vec<_> = ascribed()
    .into_iter()
    .filter(|path| !members.iter().any(|member| &member.path == path))
    .collect();

  assert!(
    stale.is_empty(),
    "`pin` ascribes a member the crate no longer has:\n{}",
    stale.join("\n")
  );
}

#[test]
fn u32_appears_only_where_a_foreign_key_round_trips() {
  let narrows = surface().narrows;
  let mut unlicensed = Vec::new();
  let mut matched = [0usize; RULE_THREE.len()];

  for narrow in &narrows {
    let site_file = narrow.site.rsplit_once(':').map_or("", |(file, _)| file);
    match RULE_THREE
      .iter()
      .position(|&(file, owner)| file == site_file && owner == narrow.owner)
    {
      Some(index) => matched[index] += 1,
      None => unlicensed.push(format!("{}: {}", narrow.site, narrow.owner)),
    }
  }

  assert!(
    unlicensed.is_empty(),
    "a `u32` outside the members rule 3 licenses is a ceiling on something painty computes. Place \
     the member with the ordered rule in README.md#numeric-widths: only rule 3 — a key painty \
     never computes, round-tripping through a contract that already spells it `u32` — gives a \
     narrow type.\n{}",
    unlicensed.join("\n")
  );

  // Exactly one each, in both directions. Zero means the exemption has outlived what it licensed
  // and is now an open door; more than one means a second narrow type sheltered under an argument
  // that was made for the first.
  let miscounted: Vec<_> = RULE_THREE
    .iter()
    .zip(matched)
    .filter(|(_, count)| *count != 1)
    .map(|((file, owner), count)| format!("{file}: {owner} matched {count} occurrences, wanted 1"))
    .collect();
  assert!(
    miscounted.is_empty(),
    "a rule 3 exemption is not single-use:\n{}",
    miscounted.join("\n")
  );
}

#[test]
fn no_position_is_built_by_a_saturating_operation() {
  let found: Vec<_> = surface()
    .saturating
    .into_iter()
    .map(|site| format!("{}: {}", site.site, site.owner))
    .collect();
  assert!(
    found.is_empty(),
    "a saturating operation on the resolution path clamps a position without saying so; use \
     checked arithmetic, or `try_from` where two units genuinely meet:\n{}",
    found.join("\n")
  );
}

#[test]
fn no_position_is_built_by_a_narrowing_cast() {
  let found: Vec<_> = surface()
    .casts
    .into_iter()
    .map(|site| format!("{}: {}", site.site, site.owner))
    .collect();
  assert!(
    found.is_empty(),
    "an `as` cast on the resolution path can narrow silently; use `try_from` and decide what a \
     value that does not fit should do:\n{}",
    found.join("\n")
  );
}

#[test]
fn the_censuses_read_something() {
  // A parser cannot mistake prose for code, which is the class of mistake the line scanner made —
  // but it can be pointed at nothing, and a census over an empty surface passes forever. So the
  // shape of what it found is asserted, not just its emptiness elsewhere.
  let surface = surface();
  assert!(
    surface.members.len() >= 20,
    "found only {} public numeric members",
    surface.members.len()
  );
  // Non-empty, not a count. A hardcoded number here would duplicate the exemption check next door
  // and would have to be edited in lockstep with it — an anti-staleness gate that itself goes
  // stale is the shape this whole file is trying to avoid.
  assert!(!surface.narrows.is_empty(), "the u32 scan found nothing");
  assert!(
    ascribed().len() >= 20,
    "read only {} ascriptions",
    ascribed().len()
  );

  // Every declaration shape the walker claims to recognise is actually reachable in this crate,
  // except the two it has no instance of — recorded so their absence is a fact rather than a
  // silence. A `pub trait` or a public field appearing later gets a member, not an exemption.
  let paths: Vec<&str> = surface.members.iter().map(|m| m.path.as_str()).collect();
  assert!(paths.contains(&"Span::new"), "an inherent function");
  assert!(
    paths.contains(&"PathSegment::Index"),
    "an enum variant payload"
  );
  assert!(
    !paths.iter().any(|path| path.starts_with("Iterator::")),
    "a trait impl's widths are the trait's, so they are not painty's to pin"
  );
}

#[test]
fn the_pinned_widths_are_the_values_the_crate_produces() {
  // The ascriptions are compile-time and touch no values, so this is what keeps them over live
  // code rather than over accessors nobody calls.
  let text = "one\ntwo\nthree\n";
  let source = Source::new(text);
  let region = source.resolve(Span::new(2, 9));
  let line = source.line(1).expect("a source has a first line");
  let drawn = region.lines().next().expect("a region is drawn somewhere");

  assert_eq!(
    (
      region.start().line(),
      region.start().column(),
      region.start().offset()
    ),
    (1, 3, 2)
  );
  assert_eq!(
    (line.number(), line.char_count(), line.column_at(1)),
    (1, 3, 2)
  );
  // Four lines in the source, counting the empty one the trailing break leaves; the region runs
  // from line 1 to the `h` on line 3.
  assert_eq!((source.line_count(), region.line_count()), (4, 3));
  assert_eq!(drawn.columns(), 3..4);
  assert_eq!(PathSegment::Index(2).to_string(), "2");
  assert_eq!(Location::new(0, Span::new(2, 9)).source(), 0);
  assert_eq!(source.len(), text.len());
  assert_eq!(
    line
      .line_break()
      .expect("the first line ends in a break")
      .byte_len(),
    1
  );
}

#[test]
fn the_last_line_number_and_the_line_count_are_the_same_number() {
  // `Source::line_count` reads the last line's ordinal rather than counting into a `usize` and
  // converting. That is only correct while line numbers start at 1 and never skip, so the identity
  // is pinned rather than assumed.
  for text in [
    "",
    "a",
    "a\n",
    "a\nb",
    "a\r\nb\rc\n",
    "\n\n\n",
    "\r\n\r\n",
    "日\n本\n",
  ] {
    let source = Source::new(text);
    let walked = source.lines().count() as u64;
    assert_eq!(source.line_count(), walked, "{text:?}");
    assert_eq!(
      source.lines().last().expect("at least one line").number(),
      walked,
      "{text:?}"
    );
  }
}
