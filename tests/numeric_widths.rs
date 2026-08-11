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
//! # THIS FILE IS AN EARLY WARNING. IT DOES NOT DECIDE.
//!
//! Read this before trusting anything below it. Everything here that reads `src/` **recognises**
//! the places a public numeric can appear, and three consecutive reviews each found another place
//! it did not recognise: a rustfmt-wrapped signature, a trait method, a name that prefixes a
//! pinned one, a macro inside an impl block, a const-generic parameter. Changing its mechanism
//! partway through — line scanning to a `syn` parse — made the recogniser better and left it a
//! recogniser. The holes share one shape: a check establishing its conclusion relative to a frame
//! it never establishes, that frame being *the set of syntactic positions a width can occupy*.
//!
//! A reader of source text cannot close that frame, because the set of positions is whatever the
//! grammar allows and the grammar grows. So the deciding check is elsewhere:
//! **`ci/public_numeric_surface.py`**, run by the `public-surface` job, which asks *rustdoc* for
//! the resolved public API and recurses over arbitrary JSON for primitives. It names no syntactic
//! position, so there is no case in it to forget.
//!
//! What this file is for, then: it runs in `cargo test`, in a second, on every developer's machine,
//! and it catches the ordinary case long before CI does. Treat a pass here as "probably fine" and
//! the `public-surface` job as "checked".
//!
//! Two things here are NOT early warnings and do decide, because they are compile-time and the
//! compiler is the authority: [`pin`], which refuses a signature, a rule or a returned borrow that
//! has changed shape, and `tests/source_borrows.rs`.
//!
//! # Why these are assertions and not scenarios
//!
//! The behaviour worth guaranteeing is that painty never reports a number that has been quietly
//! narrowed, and never publishes a width it cannot change later. The scenario that would exercise
//! a clamp needs a source with more than `u32::MAX` line breaks — over four gigabytes of text — so
//! no test is going to construct it. So the *property* is asserted instead.
//!
//! # The layers
//!
//! **Every signature, by function pointer.** [`pin`] ascribes the whole signature of every public
//! member that mentions a primitive integer, and a binding only counts as a pin when its declared
//! type really is a function pointer. A parameter's width is as frozen as a return's, and the
//! receiver's borrow is elided rather than tied to the source lifetime.
//!
//! **Which rule placed each number.** Every occurrence is spelled through its rule's alias in
//! [`rule`], so the claim is single-valued per *number* rather than per member — a member does not
//! have one rule, `Line::column_at` returns a rule 1 column and takes a rule 4 count — and the
//! compiler checks it, because a transparent alias for the wrong rule is the wrong width. This is
//! the only layer aimed at the residual that has actually occurred.
//!
//! **That the list is complete, and no longer than the surface** — early warning; the
//! `public-surface` job decides.
//!
//! **That no spelling hides a primitive** — early warning, and now closed against the positions
//! three reviews named; the `public-surface` job is what makes "closed" true rather than "closed
//! so far".
//!
//! **`u32`, by exact member.** Rule 3 is the only rule yielding a narrow type and it justifies
//! exactly four occurrences, each required to match once.
//!
//! **The arithmetic.** No saturating call and no `as` cast on the path a position is built by.
//! Nothing in rustdoc's view can see a function body, so this one has no decider behind it and is
//! the real thing here rather than a warning.
//!
//! # What still gets through
//!
//! 1. **A number placed under a rule that shares its width.** Rules 1 and 2 both give `u64`, so
//!    naming either compiles; they differ in where the number came from, not in what it is. Every
//!    other misplacement is now a type error at the occurrence itself. This is what is left of the
//!    residual that had been uncaught entirely.
//! 2. **The rule set itself being wrong.** No check can find a category nobody has thought of;
//!    rule 4 exists because two members turned out to fit none of the first three.
//! 3. **A width painty does not choose.** `Iterator::size_hint` returns `(usize, Option<usize>)`
//!    because `Iterator` says so. Excluded by name in both this file and the decider, so the
//!    exclusion is visible rather than assumed.
//!
//! # What "exact" rests on
//!
//! A line ordinal is at most one more than the text's length in bytes; Rust caps a single object
//! at `isize::MAX` bytes; so on the widest target this crate builds for, an ordinal cannot exceed
//! 2^63 and a `u64` cannot overflow. The increments are therefore plain `+`, and a debug build
//! panics if that reasoning is ever wrong — which is the direction to fail in. A saturating
//! increment would instead hand back a number indistinguishable from a real one.
//!
//! # NOT INTERPRETED UNDER MIRI
//!
//! Miri answers one question — whether an execution path has undefined behaviour — and the answer
//! is a property of the path rather than of how often it is walked. Everything here reads painty's
//! source TEXT: `syn` parses every file in [`CRATE`] and the censuses walk the trees. None
//! of painty's own paths is exercised by that, so the interpreter has nothing to have an opinion
//! about. The two tests that do call painty — [`the_pinned_widths_are_the_values_the_crate_produces`]
//! and [`the_last_line_number_and_the_line_count_are_the_same_number`] — run a fourteen-byte source
//! through accessors that `tests/resolution_invariants.rs` and the lib's own unit tests put through
//! the interpreter thousands of times over, so nothing leaves Miri's view with this file.
//!
//! Leaving it in does not cost a slow cell, it costs a red one, in two different ways. Interpreting
//! `syn` took between 2h22m and 4h58m per cell in run 31316096247 — by a wide margin the largest
//! single item in a job GitHub caps at six hours, and up to 83% of one cell's whole budget. On
//! `i686-unknown-linux-gnu` it does not finish at all: Miri hands each allocation a fresh address
//! out of the target's four-gigabyte space, a census this long exhausts it, and validation ICEs with
//! `there are no more free addresses in the address space` — 1h44m in under tree borrows, 58m under
//! stacked borrows. That is a limit of the interpreter's address allocator on a 32-bit target, not a
//! finding about painty, and no `MIRIFLAGS` entry raises it.
//!
//! `#![cfg(not(miri))]` rather than `#[cfg_attr(miri, ignore)]` per test, because the reason is a
//! property of what this file DOES and not of any test in it: it belongs where a reader looking for
//! it will be, and a test added below inherits it instead of having to remember an attribute. The
//! ICE also arrives during *evaluation* rather than compilation, so an ignored-but-present test
//! would be enough — but the whole-file form is the one that cannot go stale. `cfg(miri)` is set by
//! the interpreter and by nothing else, so `cargo test`, the coverage lane and the sanitizer lane
//! still compile and run every test here; the emptiness cannot spread beyond the two Miri lanes.
#![cfg(not(miri))]

use painty::{
  Ansi16, Color, Line, LineBreak, Location, PathSegment, Position, Region, RegionLine, Source, Span,
};
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
const CRATE: [(&str, &str); 22] = [
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
  ("src/style/color.rs", include_str!("../src/style/color.rs")),
  ("src/style/mod.rs", include_str!("../src/style/mod.rs")),
  (
    "src/terminal/ariadne.rs",
    include_str!("../src/terminal/ariadne.rs"),
  ),
  (
    "src/terminal/codespan.rs",
    include_str!("../src/terminal/codespan.rs"),
  ),
  (
    "src/terminal/detect.rs",
    include_str!("../src/terminal/detect.rs"),
  ),
  (
    "src/terminal/miette.rs",
    include_str!("../src/terminal/miette.rs"),
  ),
  (
    "src/terminal/paint.rs",
    include_str!("../src/terminal/paint.rs"),
  ),
  (
    "src/terminal/present.rs",
    include_str!("../src/terminal/present.rs"),
  ),
  (
    "src/terminal/render.rs",
    include_str!("../src/terminal/render.rs"),
  ),
  (
    "src/terminal/rustc.rs",
    include_str!("../src/terminal/rustc.rs"),
  ),
  (
    "src/terminal/mod.rs",
    include_str!("../src/terminal/mod.rs"),
  ),
  (
    "src/terminal/width.rs",
    include_str!("../src/terminal/width.rs"),
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

/// One ascription, and any primitive it spells directly instead of through its rule.
#[derive(Debug, Clone)]
struct Pin {
  path: String,
  bare: Vec<String>,
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
  /// Constructs that can dress a primitive integer in a name this scan cannot see through.
  spellings: Vec<Site>,
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

fn macro_name(kind: &str, mac: &syn::Macro) -> String {
  let name = mac
    .path
    .segments
    .last()
    .map_or_else(String::new, |segment| segment.ident.to_string());
  format!("{kind} `{name}!`")
}

fn is_public(visibility: &Visibility) -> bool {
  matches!(visibility, Visibility::Public(_))
}

/// The primitive integer a path names, if it names one.
///
/// Keyed on the LAST segment, so `core::primitive::u32` is the same answer as `u32`. Matching only
/// a single-segment path — which this did — let the qualified spelling walk past every check while
/// compiling to exactly the same type.
fn primitive(path: &syn::Path) -> Option<String> {
  let last = path.segments.last()?.ident.to_string();
  INTEGERS.contains(&last.as_str()).then_some(last)
}

/// Collects the primitive integers named anywhere inside one syntax node.
#[derive(Default)]
struct Integers(Vec<String>);

impl<'ast> Visit<'ast> for Integers {
  fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
    if node.qself.is_none()
      && let Some(found) = primitive(&node.path)
    {
      self.0.push(found);
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

  fn spelling(&mut self, what: String, span: proc_macro2::Span) {
    let site = self.site(span);
    self.out.spellings.push(Site { owner: what, site });
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
    // `pub trait Rows: Iterator<Item = usize>` publishes a width in the supertrait list, where
    // nothing else here looks and no pin can reach.
    for bound in &node.supertraits {
      let mut found = Integers::default();
      found.visit_type_param_bound(bound);
      if !found.0.is_empty() {
        self.spelling(
          format!("supertrait binding on `{}`", node.ident),
          node.ident.span(),
        );
        break;
      }
    }
    let public = core::mem::replace(&mut self.public_container, is_public(&node.vis));
    self.scoped(node.ident.to_string(), |scan| {
      visit::visit_item_trait(scan, node);
    });
    self.public_container = public;
  }

  fn visit_trait_item_const(&mut self, node: &'ast syn::TraitItemConst) {
    // A trait's members inherit the trait's publicness; there is no `pub` on them to read. This
    // handler was simply missing, which is how an associated constant of pointer width could fix
    // an API width with nothing here to notice.
    let path = format!("{}::{}", self.owner, node.ident);
    if self.public_container && !integers_in_type(&node.ty).is_empty() {
      self.member(path.clone(), node.ident.span());
    }
    self.scoped(path, |scan| visit::visit_trait_item_const(scan, node));
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

  fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
    if node.qself.is_none() && primitive(&node.path).as_deref() == Some("u32") {
      let site = self.site(node.path.span());
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

  // ── Fail closed on the spellings a primitive can hide behind ──────────────────────────────
  //
  // Everything above recognises `u32`, `usize` and their siblings by name. That recognition rests
  // on a frame nobody was checking: that a primitive integer in a public signature is *spelled*
  // like one. Four constructs break the frame, and painty uses none of them, so the honest state
  // is an empty set with no allowlist to grant an exception through.

  fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
    let path = node.ident.to_string();
    if is_public(&node.vis) && !integers_in_type(&node.ty).is_empty() {
      self.member(path.clone(), node.ident.span());
    }
    self.spelling(format!("type alias `{path}`"), node.ident.span());
    self.scoped(path, |scan| visit::visit_item_type(scan, node));
  }

  fn visit_impl_item_type(&mut self, node: &'ast syn::ImplItemType) {
    // An INHERENT associated type is a name painty invented, so it is a hazard whatever it is
    // bound to today. One in a TRAIT impl is a foreign name painty is filling in — `Iterator::Item`
    // — so the name is not painty's and only the binding matters. `type Item = Line<'a>` is
    // nothing; `type Item = usize` would make `next` hand back a public integer that no
    // `pub fn` line declares and no function pointer can pin, which is exactly the shape this
    // check exists to refuse.
    let invented = self.inherent;
    let bound_to_a_primitive = !integers_in_type(&node.ty).is_empty();
    if invented || bound_to_a_primitive {
      self.spelling(
        format!("associated type `{}::{}`", self.owner, node.ident),
        node.ident.span(),
      );
    }
    visit::visit_impl_item_type(self, node);
  }

  fn visit_trait_item_type(&mut self, node: &'ast syn::TraitItemType) {
    // An associated type in a trait painty declares is an open name an implementor fills, so this
    // scan cannot know what it will be bound to. painty declares no traits; if it ever does, that
    // is a decision to make deliberately rather than one to inherit.
    self.spelling(
      format!("open associated type `{}::{}`", self.owner, node.ident),
      node.ident.span(),
    );
    visit::visit_trait_item_type(self, node);
  }

  fn visit_use_rename(&mut self, node: &'ast syn::UseRename) {
    self.spelling(
      format!("renamed import `{} as {}`", node.ident, node.rename),
      node.rename.span(),
    );
    visit::visit_use_rename(self, node);
  }

  fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
    self.spelling(macro_name("item macro", &node.mac), node.mac.path.span());
    visit::visit_item_macro(self, node);
  }

  fn visit_impl_item_macro(&mut self, node: &'ast syn::ImplItemMacro) {
    self.spelling(
      macro_name("impl-item macro", &node.mac),
      node.mac.path.span(),
    );
    visit::visit_impl_item_macro(self, node);
  }

  fn visit_trait_item_macro(&mut self, node: &'ast syn::TraitItemMacro) {
    self.spelling(
      macro_name("trait-item macro", &node.mac),
      node.mac.path.span(),
    );
    visit::visit_trait_item_macro(self, node);
  }

  fn visit_const_param(&mut self, node: &'ast syn::ConstParam) {
    // `pub struct Page<const N: usize>` publishes a width, and no function pointer can pin a
    // const-generic parameter. Refusing it is the only posture available.
    if !integers_in_type(&node.ty).is_empty() {
      self.spelling(
        format!("const-generic parameter `{}::{}`", self.owner, node.ident),
        node.ident.span(),
      );
    }
    visit::visit_const_param(self, node);
  }

  fn visit_type_param(&mut self, node: &'ast syn::TypeParam) {
    // `pub struct Page<T = usize>` publishes a width through a default nobody has to write.
    if let Some(default) = &node.default
      && !integers_in_type(default).is_empty()
    {
      self.spelling(
        format!("defaulted type parameter `{}::{}`", self.owner, node.ident),
        node.ident.span(),
      );
    }
    visit::visit_type_param(self, node);
  }

  fn visit_item(&mut self, node: &'ast Item) {
    // Tokens syn accepted as an item without understanding them. Whatever they declare is outside
    // every check here, which is the definition of a spelling this scan cannot see through.
    if let Item::Verbatim(tokens) = node {
      self.spelling("an item syn could not parse".to_owned(), tokens.span());
    }
    visit::visit_item(self, node);
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
///
/// # A binding only counts when it is a function pointer
///
/// Requiring `let _: fn(..) -> ..` and not merely `let _ = ..` is the difference between a check
/// and a formality. A plain `let _ = Span::new;` names the same path, so it would satisfy the
/// completeness census while constraining no parameter, no return and no receiver borrow — the
/// gate would pass over a member whose widths nothing holds. So the local's declared type has to
/// be a bare function pointer, and anything else is not a pin.
fn ascribed() -> Vec<Pin> {
  #[derive(Default)]
  struct Locals(Vec<Pin>);
  impl<'ast> Visit<'ast> for Locals {
    fn visit_local(&mut self, node: &'ast syn::Local) {
      if let syn::Pat::Type(typed) = &node.pat
        && let Type::BareFn(signature) = &*typed.ty
        && let Some(init) = &node.init
        && let syn::Expr::Path(path) = &*init.expr
      {
        let joined = path
          .path
          .segments
          .iter()
          .map(|segment| segment.ident.to_string())
          .collect::<Vec<_>>()
          .join("::");
        let mut bare = Integers::default();
        bare.visit_type_bare_fn(signature);
        self.0.push(Pin {
          path: joined,
          bare: bare.0,
        });
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

/// The width each rule in `README.md#numeric-widths` gives a number it places.
///
/// # Why an alias per rule, and not a function per rule
///
/// The previous shape grouped pins into `pin_rule_1` … `pin_rule_4` and checked that the rule's
/// width appeared *somewhere* in the signature. That is a presence test, and a mixed signature
/// defeats it: `Line::column_at` returns a rule 1 column and takes a rule 4 byte offset, so its pin
/// satisfied rule 1 and rule 4 equally and could have been filed under either.
///
/// A member does not have one rule. Each *number* does. Spelling each occurrence through its rule's
/// alias makes the claim single-valued and hands the checking to the compiler: the aliases are
/// transparent, so naming the wrong rule names the wrong width and the ascription stops compiling.
/// Nothing here has to be trusted or re-derived.
///
/// Rules 1 and 2 are both `u64` and remain indistinguishable — they differ in where the number came
/// from, not in what it is. That is now visible at each occurrence rather than hidden in a grouping.
mod rule {
  /// Rule 1 — a line or column in the resolved model.
  pub type Ordinal = u64;
  /// Rule 2 — an ordinal in data the producer built.
  pub type DomainOrdinal = u64;
  /// Rule 3 — a key into a structure painty does not own, at that contract's width.
  pub type ForeignKey = u32;
  /// Rule 4 — a value painty emits into a wire format that fixes its width.
  ///
  /// An SGR escape sequence carries a colour channel in one byte, and a value outside that is not
  /// expressible in the format at all. Distinct from rule 1's refusal to adopt LSP's 32-bit cap:
  /// painty does not emit LSP — a consumer does — so that cap is somebody else's to honour, while
  /// this one is painty's own output.
  pub type Emitted = u8;
  /// Rule 5 — an index or a count of things in memory.
  pub type Count = usize;
}

/// The whole public numeric surface, one ascription per member, every number spelled by its rule.
///
/// # Why a function pointer
///
/// It fixes the parameters and the return in one line and cannot be written while forgetting an
/// argument. A parameter's width is as frozen as a return's: `Span::new` taking a count is a
/// promise to every caller who has already written one.
///
/// # Why the receiver's borrow is elided
///
/// This is subtler than it looks, and getting it wrong left a real hole. Writing
/// `fn(&'a Source<'a>, …) -> Option<Line<'a>>` looks stricter than it is: it lets the receiver
/// borrow *absorb* the source lifetime, so a regression to `-> Option<Line<'_>>` — returning a line
/// tied to the temporary `&self` borrow instead of to the text — satisfies the pin. An elided
/// lifetime in a function-pointer type is higher-ranked, which is what the regression cannot
/// satisfy; `for<'a, 's>` is the opposite error and is refused outright. Verified by planting the
/// regression against all three. `tests/source_borrows.rs` asserts the same property from the side
/// a caller feels, and does not depend on anybody getting a function-pointer type right.
fn pin<'a>(_witness: &'a ()) {
  use rule::{Count, DomainOrdinal, Emitted, ForeignKey, Ordinal};

  let _: fn(&Position) -> Ordinal = Position::line;
  let _: fn(&Position) -> Ordinal = Position::column;
  let _: fn(&Line<'a>) -> Ordinal = Line::number;
  let _: fn(&Line<'a>) -> Ordinal = Line::char_count;
  let _: fn(&Line<'a>, Count) -> Ordinal = Line::column_at;
  let _: fn(&Source<'a>) -> Ordinal = Source::line_count;
  let _: fn(&Source<'a>, Ordinal) -> Option<Line<'a>> = Source::line;
  let _: fn(&Region<'a>) -> Ordinal = Region::line_count;
  let _: fn(&RegionLine<'a>) -> core::ops::Range<Ordinal> = RegionLine::columns;

  let _: fn(DomainOrdinal) -> PathSegment<'a> = PathSegment::Index;

  // Rule 4 — emitted into an SGR escape, which fixes the width at one byte.
  let _: fn(&Ansi16) -> Emitted = Ansi16::index;
  let _: fn(Emitted) -> Option<Ansi16> = Ansi16::from_index;
  let _: fn(&Ansi16) -> (Emitted, Emitted, Emitted) = Ansi16::to_rgb;
  let _: fn(Emitted) -> Color = Color::Ansi256;
  let _: fn(Emitted, Emitted, Emitted) -> Color = Color::Rgb;

  let _: fn(ForeignKey, Span) -> Location = Location::new;
  let _: fn(ForeignKey) -> Location = Location::entire;
  let _: fn(&Location) -> ForeignKey = Location::source;

  let _: fn(Count, Count) -> Span = Span::new;
  let _: fn(Count) -> Span = Span::empty;
  let _: fn(&Span) -> Count = Span::start;
  let _: fn(&Span) -> Count = Span::end;
  let _: fn(&Span) -> Count = Span::len;
  let _: fn(&Span, Count) -> bool = Span::contains;
  let _: fn(&Position) -> Count = Position::offset;
  let _: fn(&LineBreak) -> Count = LineBreak::byte_len;
  let _: fn(&Source<'a>) -> Count = Source::len;
  let _: fn(&Source<'a>, Count) -> Line<'a> = Source::line_at;
  let _: fn(&Source<'a>, Count) -> Position = Source::position;

  // The terminal renderer's geometry, behind its feature. A cell measure is rule 1: the rule
  // covers painty's own geometry, and a distance across that geometry is in the same unit as a
  // position in it — mixing a `usize` tab width into `u64` column arithmetic is exactly the seam
  // an off-by-one hides in.
  #[cfg(feature = "terminal")]
  {
    use painty::terminal::LineCells;
    let _: fn(Line<'a>, Ordinal) -> LineCells<'a> = LineCells::new;
    let _: fn(&LineCells<'a>) -> Ordinal = LineCells::tab_width;
    let _: fn(&LineCells<'a>, Count) -> Ordinal = LineCells::column_at;
    let _: fn(&LineCells<'a>) -> Ordinal = LineCells::width;
    let _: fn(&LineCells<'a>, Count, Count) -> Ordinal = LineCells::cells_between;
    let _: fn() -> Ordinal = LineCells::default_tab_width;
    let _: fn() -> Ordinal = LineCells::max_tab_width;
    let _: fn(&LineCells<'a>, Span) -> core::ops::Range<Ordinal> = LineCells::columns_for;

    // The renderer's own geometry. A tab width is a distance across the rendered geometry and a
    // marker range is a pair of positions in it, so both are rule 1 for the same reason.
    use painty::{Theme, terminal::Terminal};
    let _: fn(Terminal<Theme>, Ordinal) -> Terminal<Theme> = Terminal::<Theme>::with_tab_width;
    let _: fn(&Terminal<Theme>, RegionLine<'a>) -> core::ops::Range<Ordinal> =
      Terminal::<Theme>::underline;
    // The ceiling on a drawn row is a distance across that same geometry, so it is rule 1 for the
    // same reason the tab width is — and it is the one number the output-size contract rests on.
    let _: fn() -> Ordinal = Terminal::<Theme>::max_rendered_width;
    // A count of source bytes the renderer will examine. Rule 1 as well: it is a measure over
    // painty's own rendered geometry rather than an index into anything, and it is the number the
    // resource bound rests on.
    let _: fn() -> Ordinal = Terminal::<Theme>::max_source_bytes;
  }

  // `painty::tokora::Adapted` has no numeric member. It had two — exact overflow counts — and
  // buying that exactness meant walking a caller's `Diagnose` impl to exhaustion, so they are
  // booleans now and leave the width rule entirely.
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

  let ascribed: Vec<String> = ascribed().into_iter().map(|pin| pin.path).collect();
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
    .map(|pin| pin.path)
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
fn every_number_in_a_pin_names_the_rule_that_placed_it() {
  // The residual at the top of this file's list is a number placed under the wrong rule, and it is
  // the only residual that has ever occurred — twice.
  //
  // It used to be checked by grouping pins into a function per rule and asking whether the rule's
  // width appeared in the signature. That was a presence test and a mixed signature defeated it:
  // `Line::column_at` returns a rule 1 column and takes a rule 4 count, so its pin satisfied both
  // and rule 1 against rule 4 was passable on the real surface.
  //
  // The claim is now per NUMBER, not per member, and the compiler checks it: each occurrence is
  // spelled through its rule's alias, and because the aliases are transparent, naming the wrong
  // rule names the wrong width and the ascription does not build. All that is left for this test
  // is to refuse the escape hatch — a pin that spells a primitive directly says nothing about which
  // rule placed it.
  let bare: Vec<_> = ascribed()
    .into_iter()
    .filter(|pin| !pin.bare.is_empty())
    .map(|pin| format!("{} spells {:?} directly", pin.path, pin.bare))
    .collect();

  assert!(
    bare.is_empty(),
    "a pin names a primitive instead of the rule that placed it, so nothing records which rule \
     that is. Use `rule::Ordinal`, `rule::DomainOrdinal`, `rule::ForeignKey` or `rule::Count` — \
     README.md#numeric-widths decides which:\n{}",
    bare.join("\n")
  );
}

#[test]
fn no_spelling_can_hide_a_primitive_integer() {
  // Every other check here recognises an integer by name, which rests on the assumption that a
  // primitive in a public signature is spelled like one. These four constructs break that
  // assumption, painty uses none of them, and there is deliberately no allowlist: an exception
  // has to be argued for by editing this test, not granted by adding a row.
  //
  // A type alias or a renamed import can put any name on `u32`. An item macro can declare members
  // this scan never sees, because it reads the invocation and not the expansion. Tokens `syn`
  // accepts as an item without understanding are outside every check by definition.
  //
  // The alternative is the compiler's own view of the resolved surface — rustdoc JSON, or a crate
  // built on it. That is the decider the way `syn` was the decider over line scanning, and at this
  // crate's size it costs more than it buys: it is nightly-only (verified — stable rejects
  // `-Z unstable-options`), its schema is unstable across nightlies while this repository's weekly
  // schedule build picks up a new one, and reaching it from a test means a nested `cargo`
  // invocation contending for the same build lock. Against that: twenty-six numeric members and
  // zero aliases. What it would additionally close is a FOREIGN type that is secretly an integer
  // alias, which is residual item 3 and needs painty to put somebody else's type in a public
  // signature first.
  let found: Vec<_> = surface()
    .spellings
    .into_iter()
    .map(|site| format!("{}: {}", site.site, site.owner))
    .collect();
  assert!(
    found.is_empty(),
    "this can dress a primitive integer in a name the surface scan cannot see through. Resolve it      here — teach the scan to follow it — or remove it; there is no allowlist on purpose:\n{}",
    found.join("\n")
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
