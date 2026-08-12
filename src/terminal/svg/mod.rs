//! The second surface: the same render, as an image of a terminal.
//!
//! # Which of the two things called "SVG output" this is, and why
//!
//! The design named two, at different layers, and deferred the choice until the HTML renderer
//! existed so that it would be decided by evidence. HTML landed; this is what it decided.
//!
//! **Native SVG — real geometry, variable-width fonts, curved connectors — is not buildable
//! inside this crate's boundary, and the reason is a fact about the medium rather than a cost.**
//! SVG has no layout engine. Every glyph run is placed at an absolute coordinate the *author*
//! computes, so putting a label under a span of variable-width text requires the advance width of
//! that text in the font that will draw it. painty's whole dependency budget is `anstyle`,
//! `unicode-width` and `unicode-segmentation`; §5 of the design says it is not a font or layout
//! engine; and every route around the measurement collapses. `<foreignObject>` is HTML inside a
//! box that no rasteriser outside a browser renders, and is strictly worse than the HTML renderer
//! that already exists. `textLength` needs the number it was supposed to avoid computing. And
//! assuming a fixed advance is not variable-width geometry at all — it is a cell grid, which is
//! the thing below.
//!
//! The spec's argument for Native SVG proves a different proposition than the one it draws.
//! "`Presentation` is denominated in display cells, so SVG cannot be a style of the terminal" is
//! correct, and it establishes only that SVG is not a `Presentation`. Buildability was the missing
//! premise.
//!
//! **Terminal-as-SVG is the one that survives, but not at the layer the spec put it.** It called
//! for serialising "the cell grid and colours `Terminal` already produces". There is no cell grid:
//! [`Terminal::render`](super::Terminal::render) streams characters and SGR into a
//! [`fmt::Write`], the cell model exists transiently inside [`LineCells`](super::LineCells) and is
//! never materialised, and [`Plan`](super::render::Plan) is lines and byte spans rather than
//! cells. So "already produces" names an artefact that does not exist, and serialising what does
//! exist — the finished ANSI — is the worse of the two shapes available:
//!
//! * it recovers **colours, not roles**, and a resolved colour written into a document is the
//!   inline style this crate declined for HTML; and
//! * it recovers **nothing at all** at [`ColorCapability::None`](super::ColorCapability::None),
//!   which is what [`Terminal::plain`](super::Terminal::plain) selects, because there are no
//!   escapes in that stream to parse.
//!
//! The seam that does carry roles is one layer down and was already there:
//! [`Painter`](super::paint::Painter). Every byte of a render goes through it, every byte that
//! means anything goes through a hook that knows its [`Role`], and no `Presentation` ever touches
//! a writer. So the whole of this output's cost is one implementor of
//! [`Surface`](super::paint::Surface) — and "every style gets SVG for free" is then literally
//! true, because the plan, the style, the elision and the geometry are the same call.
//!
//! # What building HTML actually taught, which is what settled it
//!
//! HTML's own documentation says the two renderers share **layer 2** and **the elision rule**, and
//! explicitly not the terminal's plan. That is exact, and it is the argument against a third peer
//! rather than for one: what HTML did instead of reusing the plan was **re-derive it** —
//! `Drawable`, the block order, the ascending stop walk, the anchors, the elision loop, the label
//! order under a line, and "does a bracket open here" are all written twice, once with a sort and
//! once as an allocation-free selection scan at `O(k²)`. The extraction of
//! [`elide`](crate::elide) is what a rule costs when it is duplicated: two copies of one piece of
//! arithmetic put a line in neither output, and it shipped for a review round.
//!
//! So the peer layer generalised to layer 2 and one function, and no further. HTML was cheap for a
//! reason SVG does not share — it has no cells **and** it delegates every placement to a browser —
//! and a third peer would be a third copy of the plan, a third chance at that defect, and a third
//! side for `the_two_renderers_draw_the_same_rows` to hold.
//!
//! # The vocabulary is HTML's, deliberately
//!
//! One class per [`Role`], with the names `painty::html` already uses, so an author theming both
//! outputs writes one list of selectors. What differs is where the appearance comes from: an HTML
//! fragment has an embedder's stylesheet and this does not, so the document carries a `<style>`
//! element generated from the caller's [`Palette`] — which is a stylesheet, not an inline style,
//! and an outer sheet still overrides it.
//!
//! | class | on | [`Role`] |
//! |---|---|---|
//! | `painty-error`, `painty-warning`, `painty-advice` | the severity word | [`Role::Severity`] |
//! | `painty-code` | the machine identifier | [`Role::Code`] |
//! | `painty-gutter` | the bars and brackets | [`Role::Gutter`] |
//! | `painty-number` | a line number | [`Role::LineNumber`] |
//! | `painty-text` | the source itself | [`Role::SourceText`] |
//! | `painty-primary`, `painty-secondary` | a marker, a connector, a label | [`Role::PrimaryLabel`], [`Role::SecondaryLabel`] |
//! | `painty-help` | what the reader can do | [`Role::Help`] |
//!
//! # Two passes, because an SVG declares its size in its first element
//!
//! The width and height of the document are functions of the whole render, and they are attributes
//! of the root element. The alternative is materialising the document and measuring it, which
//! takes the decision to refuse away from the caller's writer — the thing
//! [`Terminal::render`](super::Terminal::render) goes out of its way to preserve, and what
//! `tests/writer_discipline.rs` holds it to. So the render runs twice: once into [`Extent`], which
//! writes nothing and cannot refuse, and once into [`Document`], which streams. Nothing is ever
//! held.
//!
//! What that costs is the plan built twice, stated rather than hidden. What it assumes is that a
//! caller's [`fmt::Display`] writes the same thing twice; one that does not gets a `viewBox` that
//! disagrees with its contents, which is a wrong size and not a wrong marking.
//!
//! # What it does not carry
//!
//! * **A background colour.** [`Style::background`](crate::Style::background) has no equivalent
//!   for SVG text: drawing one means a rectangle behind the run, and a run's width is not known
//!   until it has been written — which is the buffering this surface refuses. painty's three
//!   built-in themes set no background, so the default path loses nothing.
//! * **Exact placement in a font that is not monospace.** Each row is one `<text>` with
//!   `textLength`, so a row of a *monospace* face lands on its cells whatever that face's advance
//!   ratio is — the correction is uniform and every advance is equal, so it is exact rather than
//!   close. A proportional face is outside the model, exactly as it is for the terminal this
//!   serialises. The residual is a row mixing zero-width or double-width clusters with a face
//!   whose ratio is not the nominal one, where the uniform per-glyph correction is not the
//!   per-cell one; it is bounded by the ratio error and is sub-pixel on every face in the stack
//!   below.
//! * **A capability.** [`ColorCapability`](super::ColorCapability) says which escape sequences a
//!   *terminal* may be sent. This medium has none, so the question is not asked of it and the
//!   palette is read at full fidelity — which is why `Terminal::plain()`, whose capability is
//!   `None`, still produces a coloured image. A caller who wants no colour asks for
//!   [`Theme::monochrome`](crate::Theme::monochrome), which is the same answer
//!   [`Style::without_color`](crate::Style::without_color) already documents.

#[cfg(test)]
mod tests;

use core::fmt;

use super::{paint::Surface, width::cells_from};
use crate::{Color, Palette, Role, Severity, Style, escape::Escaped};

/// The cell width, in user units.
///
/// Nominally `0.6em` at [`FONT_SIZE`], which is the ratio most monospace faces use. It does not
/// have to be any face's actual ratio: every row carries `textLength`, so the glyphs are placed on
/// these cells rather than on the font's.
const ADVANCE: u64 = 8;

/// The distance between two rows' baselines.
const LINE_HEIGHT: u64 = 17;

/// The type size the stylesheet asks for.
const FONT_SIZE: u64 = 13;

/// How far a row's baseline sits below the top of its line box.
const BASELINE: u64 = 13;

/// The margin around the whole image, so that a descender or a bold stem is not clipped by the
/// viewport.
const PAD: u64 = 8;

/// The font stack, in the order a viewer should try it.
///
/// Every entry is a fixed-advance face, which is what the placement model needs; the generic
/// `monospace` at the end is what a viewer with none of the named ones falls back to.
const FONT_STACK: &str =
  "ui-monospace,SFMono-Regular,Menlo,Consolas,\"DejaVu Sans Mono\",\"Liberation Mono\",monospace";

/// The CSS class one [`Role`] is written as.
///
/// Exhaustive, which is the point of routing every class through it: [`Role`] is
/// `#[non_exhaustive]` for other crates and closed inside this one, so a role added later fails to
/// compile **here** until this surface has decided what it is called — and that failure is what
/// sends its author to [`ROLES`], which a table alone could not do.
///
/// The names are [`painty::html`](crate::html)'s, so the two markup outputs are themed by one list
/// of selectors.
const fn class(role: Role) -> &'static str {
  match role {
    Role::Severity(Severity::Error) => "painty-error",
    Role::Severity(Severity::Warning) => "painty-warning",
    Role::Severity(Severity::Advice) => "painty-advice",
    Role::PrimaryLabel => "painty-primary",
    Role::SecondaryLabel => "painty-secondary",
    Role::Gutter => "painty-gutter",
    Role::LineNumber => "painty-number",
    Role::SourceText => "painty-text",
    Role::Help => "painty-help",
    Role::Code => "painty-code",
  }
}

/// Every role the stylesheet is written for.
///
/// A list rather than something derived, because there is nothing to derive it from: a `match` is
/// total over a type and cannot be iterated, so [`class`] cannot produce this and this cannot
/// prove [`class`] total. The two directions are covered by two different things and it is worth
/// being exact about which: a role ADDED to [`Role`] fails to compile in [`class`], and a role
/// added to [`class`] but left out of here reaches the document with no rule — which
/// `every_class_the_document_uses_is_one_the_stylesheet_knows` reads off the finished document
/// rather than off this array.
const ROLES: [Role; 10] = [
  Role::Severity(Severity::Error),
  Role::Severity(Severity::Warning),
  Role::Severity(Severity::Advice),
  Role::PrimaryLabel,
  Role::SecondaryLabel,
  Role::Gutter,
  Role::LineNumber,
  Role::SourceText,
  Role::Help,
  Role::Code,
];

/// The concrete colour an SVG has to name for a colour ANSI only names.
///
/// The sixteen are [`Ansi16::to_rgb`](crate::Ansi16::to_rgb), which exists for exactly this — a
/// medium with no palette needing *some* value. The 256 are the inverse of the quantisation
/// [`Color::to_ansi256`](crate::Color::to_ansi256) performs: a 6×6×6 cube whose levels are 0 and
/// then 95 upwards in steps of 40, and a 24-step grey ramp from 8 in steps of 10.
///
/// It is one terminal's palette written down, and it is named as one. A terminal resolves these
/// indices against a scheme the user chose; a file has to carry a value, and this is the scheme
/// `xterm` ships and every other emulator is measured against.
const fn rgb_of(colour: Color) -> (u8, u8, u8) {
  match colour {
    Color::Ansi16(sixteen) => sixteen.to_rgb(),
    Color::Ansi256(index) => ansi256_to_rgb(index),
    Color::Rgb(red, green, blue) => (red, green, blue),
  }
}

/// One 256-palette index as a colour.
const fn ansi256_to_rgb(index: u8) -> (u8, u8, u8) {
  /// The cube's six levels.
  const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

  if index < 16 {
    // Total below sixteen; the arm is unreachable and black is the inert answer.
    return match crate::Ansi16::from_index(index) {
      Some(colour) => colour.to_rgb(),
      None => (0, 0, 0),
    };
  }
  if index < 232 {
    let cube = index - 16;
    return (
      LEVELS[(cube / 36) as usize],
      LEVELS[((cube % 36) / 6) as usize],
      LEVELS[(cube % 6) as usize],
    );
  }
  // 8, 18, … 238: twenty-four greys, and the last index lands on 238 rather than white.
  let grey = 8 + (index - 232) * 10;
  (grey, grey, grey)
}

/// What the first pass learns: how many cells each row of the render occupies.
///
/// Writes nothing, so it cannot refuse and cannot be refused. It is the ONLY thing that measures:
/// the second pass places nothing itself, it reads a row's width back out of here and hands it to
/// `textLength`. One measure and one reader is what makes the viewport, the rows and the glyphs
/// agree by construction rather than by three readings of one rule.
///
/// The measure is [`cells_from`], which is [`LineCells`](super::LineCells)' own walk — so the cells
/// a row is stretched to are the cells the terminal counted for the source in it, and a marker row
/// lands under the glyph it was placed under.
pub(super) struct Extent {
  column: u64,
  rows: Vec<u64>,
}

impl Extent {
  pub(super) const fn new() -> Self {
    Self {
      column: 0,
      rows: Vec::new(),
    }
  }

  /// Every row's width, including a last row the render did not end with a break.
  pub(super) fn finish(mut self) -> Vec<u64> {
    if self.column > 0 {
      self.rows.push(self.column);
    }
    self.rows
  }
}

impl fmt::Write for Extent {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    for (index, segment) in text.split('\n').enumerate() {
      if index > 0 {
        self.rows.push(self.column);
        self.column = 0;
      }
      self.column += cells_from(segment);
    }
    Ok(())
  }
}

impl Surface for Extent {
  fn open(&mut self, _role: Role, _style: Style) -> fmt::Result {
    Ok(())
  }

  fn close(&mut self, _role: Role, _style: Style) -> fmt::Result {
    Ok(())
  }
}

/// The document being written, and the only thing that decides what reaches the writer unread.
///
/// One `<text>` element per row and one `<tspan>` per styled run inside it, both opened lazily —
/// so a row with nothing on it costs no element and a run with nothing in it costs no element
/// either. Nothing is buffered: a run's `x` is not written, because the row's `textLength` places
/// every glyph, and the row's width came from the pass before.
pub(super) struct Document<'a> {
  out: &'a mut dyn fmt::Write,
  widths: &'a [u64],
  row: usize,
  /// The role in effect, which survives a row break: a style writing a run that spans one is rare
  /// but the surface must not lose it.
  role: Option<Role>,
  /// Whether a `<tspan>` is open, and whether a `<text>` is. Set **after** the element has been
  /// written and not before — see [`open_span`](Self::open_span).
  span: bool,
  line: bool,
  /// Whether the writer has already refused something.
  ///
  /// # What a refusal ends, and why this is stricter than the terminal
  ///
  /// Once a writer has said no, this document is over: every later write returns the error without
  /// offering the writer a byte. So **what a refusing writer received is a prefix of what an
  /// accepting one would have received**, which is the strongest thing a truncated document can
  /// promise and is what `a_writer_that_refuses_stops_the_document` holds.
  ///
  /// Without it the promise is false in two ways, and the second is the one that is not obvious.
  /// A later write is SHORTER than the one that was refused, so a bounded writer takes it and the
  /// document acquires a hole rather than an end. And [`styled_with`] deliberately offers the
  /// closing half of a run even when the body failed — which the terminal needs, because an SGR
  /// opener it never resets escapes into the terminal's own state. An unclosed `<tspan>` in a
  /// string the caller has just been handed an `Err` for has no equivalent reach, so this surface
  /// can answer the balance call by writing nothing, and does.
  ///
  /// [`styled_with`]: super::paint::Painter::styled_with
  poisoned: bool,
}

impl<'a> Document<'a> {
  pub(super) fn new(out: &'a mut dyn fmt::Write, widths: &'a [u64]) -> Self {
    Self {
      out,
      widths,
      row: 0,
      role: None,
      span: false,
      line: false,
      poisoned: false,
    }
  }

  /// painty's own markup, written as it is — and nothing at all once the writer has refused.
  fn markup(&mut self, text: &str) -> fmt::Result {
    if self.poisoned {
      return Err(fmt::Error);
    }
    self.out.write_str(text).inspect_err(|_| {
      self.poisoned = true;
    })
  }

  /// A number painty computed, which no caller can put a `<` into.
  fn number(&mut self, number: u64) -> fmt::Result {
    if self.poisoned {
      return Err(fmt::Error);
    }
    self
      .out
      .write_fmt(format_args!("{number}"))
      .inspect_err(|_| {
        self.poisoned = true;
      })
  }

  /// Opens the row's element, if it has not been opened already.
  ///
  /// `textLength` is what places the glyphs: the row is stretched to exactly its cells, and since
  /// every advance in a monospace face is equal, a uniform correction leaves each glyph on its own
  /// cell. A row whose width the first pass did not reach — which no render produces, the two
  /// passes being the same call — is written without it rather than with a fabricated one.
  fn open_line(&mut self) -> fmt::Result {
    if self.line {
      return Ok(());
    }
    self.markup("<text xml:space=\"preserve\" x=\"")?;
    self.number(PAD)?;
    self.markup("\" y=\"")?;
    let above = LINE_HEIGHT.saturating_mul(u64::try_from(self.row).unwrap_or(u64::MAX));
    self.number(PAD + BASELINE + above)?;
    if let Some(width) = self
      .widths
      .get(self.row)
      .copied()
      .filter(|width| *width > 0)
    {
      self.markup("\" textLength=\"")?;
      self.number(width.saturating_mul(ADVANCE))?;
      self.markup("\" lengthAdjust=\"spacing")?;
    }
    self.markup("\">")?;
    // After the element, not before it. Set first, a refused opener leaves the surface believing
    // something is open, and the balance call that follows writes a closer for an element the
    // writer never saw — which is what the prefix property found the first time it was asserted.
    // [`poisoned`](Self::poisoned) now stops that too, so planting this alone reddens nothing; the
    // ordering stays because it is what makes the flag true rather than what makes it harmless.
    self.line = true;
    Ok(())
  }

  /// Opens the run's element, if a role is in effect and it has not been opened already.
  fn open_span(&mut self) -> fmt::Result {
    if self.span {
      return Ok(());
    }
    let Some(role) = self.role else {
      return Ok(());
    };
    self.markup("<tspan class=\"")?;
    self.markup(class(role))?;
    self.markup("\">")?;
    self.span = true;
    Ok(())
  }

  fn close_span(&mut self) -> fmt::Result {
    if !self.span {
      return Ok(());
    }
    self.span = false;
    self.markup("</tspan>")
  }

  /// The document, once the writer has refused, is over — see [`poisoned`](Self::poisoned).
  fn write_escaped(&mut self, text: &str) -> fmt::Result {
    if self.poisoned {
      return Err(fmt::Error);
    }
    fmt::Write::write_str(&mut Escaped(self.out), text).inspect_err(|_| {
      self.poisoned = true;
    })
  }

  fn close_line(&mut self) -> fmt::Result {
    self.close_span()?;
    if !self.line {
      return Ok(());
    }
    self.line = false;
    self.markup("</text>\n")
  }

  /// Whatever is left open when the render ends.
  ///
  /// A render that ends without a trailing break still has a row on the page, and a writer that
  /// refused mid-row leaves elements open — which the error says, on the same terms as the HTML
  /// renderer: an unclosed element in a string the caller has just been handed an `Err` for has no
  /// reach.
  pub(super) fn finish(&mut self) -> fmt::Result {
    self.close_line()
  }
}

impl fmt::Write for Document<'_> {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    for (index, segment) in text.split('\n').enumerate() {
      if index > 0 {
        self.close_line()?;
        self.row += 1;
      }
      if segment.is_empty() {
        continue;
      }
      self.open_line()?;
      self.open_span()?;
      // Everything is escaped, painty's own glyphs included. Which context a writer three calls
      // down is in is not a question it can answer, and `-->` carries a `>` — so one rule, applied
      // everywhere, exactly as the escaper's own documentation argues.
      self.write_escaped(segment)?;
    }
    Ok(())
  }
}

impl Surface for Document<'_> {
  fn open(&mut self, role: Role, _style: Style) -> fmt::Result {
    self.close_span()?;
    self.role = Some(role);
    Ok(())
  }

  fn close(&mut self, _role: Role, _style: Style) -> fmt::Result {
    self.close_span()?;
    self.role = None;
    Ok(())
  }
}

/// The root element and the stylesheet, written from the caller's palette.
///
/// The size is the extent the first pass measured, plus a margin at each edge. `viewBox` as well as
/// `width` and `height`, because the first is what lets the image scale into a column of a README
/// and the second pair is what stops a viewer with no container from falling back to 300 by 150.
pub(super) fn open_document(
  out: &mut impl fmt::Write,
  widths: &[u64],
  palette: &dyn Palette,
) -> fmt::Result {
  let cells = widths.iter().copied().max().unwrap_or(0);
  let width = PAD.saturating_mul(2) + cells.saturating_mul(ADVANCE);
  let rows = u64::try_from(widths.len()).unwrap_or(u64::MAX);
  let height = PAD.saturating_mul(2) + LINE_HEIGHT.saturating_mul(rows);
  out.write_fmt(format_args!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" \
     width=\"{width}\" height=\"{height}\">\n"
  ))?;
  out.write_fmt(format_args!(
    "<style>\ntext{{font-family:{FONT_STACK};font-size:{FONT_SIZE}px}}\n"
  ))?;
  for role in ROLES {
    rule(out, role, palette.style(role))?;
  }
  out.write_str("</style>\n")
}

/// One role's rule, and nothing at all for a role the palette asks nothing of.
///
/// [`Style::background`](crate::Style::background) is the one property with no equivalent — see
/// the module documentation — and it is dropped rather than approximated.
fn rule(out: &mut impl fmt::Write, role: Role, style: Style) -> fmt::Result {
  // `is_plain` is not the test: a background is the one property this medium drops, so a style
  // asking for nothing else has no rule to write.
  if style.foreground().is_none() && !style.bold() && !style.italic() && !style.underline() {
    return Ok(());
  }
  out.write_fmt(format_args!(".{}{{", class(role)))?;
  let mut written = false;
  if let Some(colour) = style.foreground() {
    let (red, green, blue) = rgb_of(colour);
    out.write_fmt(format_args!("fill:#{red:02x}{green:02x}{blue:02x}"))?;
    written = true;
  }
  for (asked, property) in [
    (style.bold(), "font-weight:bold"),
    (style.italic(), "font-style:italic"),
    (style.underline(), "text-decoration:underline"),
  ] {
    if asked {
      if written {
        out.write_str(";")?;
      }
      out.write_str(property)?;
      written = true;
    }
  }
  out.write_str("}\n")
}

/// The end of the document.
pub(super) fn close_document(out: &mut impl fmt::Write) -> fmt::Result {
  out.write_str("</svg>\n")
}
