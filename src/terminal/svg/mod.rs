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
//! writes nothing and cannot refuse, and once into [`Document`], which streams. No document is
//! ever held.
//!
//! What that costs is the plan built twice, stated rather than hidden. What it does **not** cost is
//! the caller's [`fmt::Display`] being asked twice: a `Display` is allowed to be stateful, so two
//! walks over one render must not be two questions to one caller. The message is formatted once
//! into a string and both passes read it — see
//! [`Terminal::render_svg`](super::Terminal::render_svg), which is also where what the first pass
//! spends before the writer is consulted is written down.
//!
//! Asking once is not the same as spending once. The string is **held** while both passes run, and
//! a `&dyn Display` is two words that can synthesize any amount of it, so the one input here that
//! the caller did not have to allocate is also the one this surface cannot stream past a writer
//! that would refuse it. [`Captured`] is the ceiling on that, and
//! [`Terminal::max_svg_message_bytes`](super::Terminal::max_svg_message_bytes) is the number.
//!
//! # What it does not carry
//!
//! * **A background colour.** [`Style::background`](crate::Style::background) has no equivalent
//!   for SVG text: drawing one means a rectangle behind the run, and a run's width is not known
//!   until it has been written — which is the buffering this surface refuses. painty's three
//!   built-in themes set no background, so the default path loses nothing.
//! * **A glyph that fits its cell in a face whose advance is not this one's.** *Placement* is
//!   exact in any face whatever, because every unit states an absolute `x` and no font metric
//!   enters the arithmetic — see [`Document`]. What a face's ratio still decides is the ink: a
//!   glyph drawn wider than [`ADVANCE`] overhangs the cell it starts in, and one drawn narrower
//!   leaves a gap in it. Bounded by the ratio error, sub-pixel on every face in the stack below,
//!   and — unlike a caret in the wrong cell — visible for what it is.
//! * **A capability.** [`ColorCapability`](super::ColorCapability) says which escape sequences a
//!   *terminal* may be sent. This medium has none, so the question is not asked of it and the
//!   palette is read at full fidelity — which is why `Terminal::plain()`, whose capability is
//!   `None`, still produces a coloured image. A caller who wants no colour asks for
//!   [`Theme::monochrome`](crate::Theme::monochrome), which is the same answer
//!   [`Style::without_color`](crate::Style::without_color) already documents.

#[cfg(test)]
mod tests;

use core::fmt;

use super::{paint::Surface, width::measured};
use crate::{Color, Palette, Role, Severity, Style, escape::Escaped};

/// The cell width, in user units.
///
/// Nominally `0.6em` at [`FONT_SIZE`], which is the ratio most monospace faces use. It does not
/// have to be any face's actual ratio: every unit is placed at an absolute `x` that is a multiple
/// of this, so the grid is this number's and never the font's.
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

/// What the first pass learns: how far the render reaches, right and down.
///
/// Writes nothing, so it cannot refuse and cannot be refused.
///
/// # Two numbers, where this used to keep a row of them
///
/// It held a `Vec<u64>` — one width per row — because the second pass stretched each row to its
/// total and had to read that total back. Nothing reads a row total any more, so the vector is
/// gone, and with it an allocation that grew with the render and stayed alive underneath the second
/// pass's [`Plan`](super::render::Plan).
///
/// That is not a saving found beside the placement change; it is the same change. A row total is
/// the only thing a row-wide correction can be computed from, and a document that states every
/// unit's position has nothing left to want it for.
pub(super) struct Extent {
  column: u64,
  widest: u64,
  rows: u64,
}

impl Extent {
  pub(super) const fn new() -> Self {
    Self {
      column: 0,
      widest: 0,
      rows: 0,
    }
  }

  /// The widest row and how many rows there are, counting a last row the render did not end with a
  /// break.
  pub(super) const fn finish(self) -> (u64, u64) {
    if self.column == 0 {
      return (self.widest, self.rows);
    }
    let widest = if self.column > self.widest {
      self.column
    } else {
      self.widest
    };
    (widest, self.rows.saturating_add(1))
  }

  const fn break_row(&mut self) {
    if self.column > self.widest {
      self.widest = self.column;
    }
    self.rows = self.rows.saturating_add(1);
    self.column = 0;
  }
}

impl fmt::Write for Extent {
  /// Defined in terms of [`advance`](Surface::advance), so that painty's own frame is measured by
  /// the same rule as everything a caller supplied and there is one place a cell is counted.
  fn write_str(&mut self, text: &str) -> fmt::Result {
    for (index, segment) in text.split('\n').enumerate() {
      if index > 0 {
        self.break_row();
      }
      for (cluster, cells) in measured(segment) {
        self.advance(cluster, cells)?;
      }
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

  fn advance(&mut self, _text: &str, cells: u64) -> fmt::Result {
    self.column = self.column.saturating_add(cells);
    Ok(())
  }
}

/// The document being written, and the only thing that decides what reaches the writer unread.
///
/// One `<text>` element per row, one `<tspan class>` per styled run inside it, and one
/// `<tspan x>` per placement unit inside that — each opened lazily, so a row with nothing on it
/// costs no element and a run with nothing in it costs no element either. Nothing is buffered.
///
/// # Every unit states its own position, and the alternative is not a tuning
///
/// A row used to be one `<text>` with a `textLength` equal to its cells, stretched by
/// `lengthAdjust="spacing"`. That is a **row-wide** correction, and it cannot express a cell grid
/// for two independent reasons.
///
/// The first is the one a mixed row shows. The correction is distributed between glyphs, not
/// applied per cell, so on a row of ASCII and CJK the slack a wide fallback glyph creates is spread
/// in front of narrow glyphs that were already where they belonged. Marker rows are corrected
/// separately from the source rows they point at, so two rows whose totals are both right can still
/// disagree cell for cell.
///
/// The second holds even on a row where every advance is equal, and it is the one that settles it.
/// `textLength` fixes the SUM of a run's advances; where the interior glyphs land is then whatever
/// the renderer's distribution rule says, and "the advance values are adjusted" admits two. Spread
/// the slack across all *n* advances and glyph *i* lands at `i·L/n`, which is the grid. Spread it
/// across the *n−1* spacings between the glyphs and glyph *i* lands at `i·(L−a)/(n−1)`, which
/// equals the grid only when `L = n·a` — only, that is, when there was no correction to make. Both
/// readings put the run's total at `L`, so the row is the right width under either and its cells
/// are right under only one. A mechanism whose accuracy depends on which reading the viewer
/// implements is not a mechanism to tune.
///
/// So position is **stated**. Each unit is a text chunk of its own at an absolute `x`, computed
/// from the cells [`LineCells`](super::LineCells) assigned to everything before it on the row.
/// Nothing accumulates, nothing is corrected, and no font metric enters: the *placement* is exact
/// in any face whatever, and what a face's advance ratio still decides is only whether a glyph
/// slightly overhangs its cell or leaves a gap in it — ink, not position.
///
/// Two consequences worth stating rather than discovering. A cluster is one chunk, so the shaping
/// inside it — a ZWJ sequence, a base and its combining marks — is the viewer's to do and comes out
/// as one glyph. And a ligature *across* clusters is broken, which is
/// [`LineCells`](super::LineCells)' model rather than a loss: two code points the caller can span
/// separately are two addressable cells.
///
/// # What it costs
///
/// About twenty-two bytes a cell, against about one before, which over this file's corpus is a
/// document of two and a half to seven kilobytes where it used to be one to two — the stylesheet
/// being most of the floor either way.
///
/// The cheaper encodings do not survive the constraint above this one. SVG can position a whole run
/// from a list — `x="8 16 24"` — but an attribute has to be written before its element's content,
/// so a surface would have to know how long the run is *before* writing any of it, and a streaming
/// surface is told a unit at a time. Buying those bytes means buffering a run, which is the thing
/// this exists not to do.
pub(super) struct Document<'a> {
  out: &'a mut dyn fmt::Write,
  row: u64,
  /// The cells already spent on the row being written, which is where the next unit goes.
  column: u64,
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
  pub(super) fn new(out: &'a mut dyn fmt::Write) -> Self {
    Self {
      out,
      row: 0,
      column: 0,
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
  /// Carries the baseline and nothing else. The row has no `x` of its own and no width: every unit
  /// inside it states an absolute position, so there is no total for this element to hold and
  /// nothing for the first pass to have told it.
  fn open_line(&mut self) -> fmt::Result {
    if self.line {
      return Ok(());
    }
    self.markup("<text xml:space=\"preserve\" y=\"")?;
    self.number(
      LINE_HEIGHT
        .saturating_mul(self.row)
        .saturating_add(PAD + BASELINE),
    )?;
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
  ///
  /// Two filters, and they answer two different questions. [`Escaped`] takes the characters that
  /// would stop being text and become markup; [`Representable`] takes the ones that are not
  /// permitted in an XML document under any spelling, entity references included.
  fn write_escaped(&mut self, text: &str) -> fmt::Result {
    if self.poisoned {
      return Err(fmt::Error);
    }
    let mut representable = Representable(&mut *self.out);
    fmt::Write::write_str(&mut Escaped(&mut representable), text).inspect_err(|_| {
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
  /// A row break is the one thing a unit cannot be, so it is the one thing this handles: everything
  /// between two of them is segmented by [`measured`] and placed by
  /// [`advance`](Surface::advance), which is the same path text that arrived already measured
  /// takes.
  fn write_str(&mut self, text: &str) -> fmt::Result {
    for (index, segment) in text.split('\n').enumerate() {
      if index > 0 {
        self.close_line()?;
        self.row = self.row.saturating_add(1);
        self.column = 0;
      }
      for (cluster, cells) in measured(segment) {
        self.advance(cluster, cells)?;
      }
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

  /// One unit, at the cell the terminal put it in.
  ///
  /// A zero-cell unit — a lone combining mark, a joiner that began a cluster — is written like any
  /// other and moves nothing, which is what [`LineCells`](super::LineCells) says a zero-width
  /// cluster does.
  fn advance(&mut self, text: &str, cells: u64) -> fmt::Result {
    self.open_line()?;
    self.open_span()?;
    self.markup("<tspan x=\"")?;
    self.number(PAD.saturating_add(self.column.saturating_mul(ADVANCE)))?;
    self.markup("\">")?;
    // Everything is escaped, painty's own glyphs included. Which context a writer three calls
    // down is in is not a question it can answer, and `-->` carries a `>` — so one rule, applied
    // everywhere, exactly as the escaper's own documentation argues.
    self.write_escaped(text)?;
    self.markup("</tspan>")?;
    // After the unit reached the writer. A refused write must not move the column, or the rest of
    // the row would be placed against cells nothing was drawn in — and the refusal path is the one
    // where a wrong column is least likely to be looked at.
    self.column = self.column.saturating_add(cells);
    Ok(())
  }
}

/// A writer that replaces every scalar an XML document cannot carry.
///
/// # The whole of XML 1.0's `Char` production, because two values would be a sample
///
/// ```text
/// Char ::= #x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF]
/// ```
///
/// Everything outside it is unrepresentable — not merely discouraged, and not fixable with a
/// numeric reference, since `&#xFFFF;` is as ill-formed as the character itself. A document
/// carrying one does not render badly; it does not parse, so a single such scalar anywhere in a
/// source line, a message, a label, a code, an origin or a help string costs the whole image.
///
/// Read against what a Rust `char` can be, the complement is a closed list of three parts and this
/// enumerates it rather than sampling it:
///
/// * **C0 except tab, newline and carriage return.** Substituted before this, by
///   [`control_picture`](super::width::control_picture) — which is why the escaper beside this one
///   says a C0 character is not its problem. True, and true of C0 alone: the sanitizer is a table of
///   *pictures*, and there is no picture for a noncharacter.
/// * **The surrogates, `#xD800`–`#xDFFF`.** Unreachable: a `char` is a Unicode scalar value, so no
///   `&str` this crate can be handed contains one. Said out loud because a reader checking this
///   list against the production has to know the omission is a proof and not an oversight.
/// * **`#xFFFE` and `#xFFFF`.** Reachable, from any caller string and from source text, and the two
///   the range above ends at `#xFFFD` to exclude.
///
/// The rest of Unicode's noncharacters — `#xFDD0`–`#xFDEF`, and the pair at the end of every
/// astral plane — are *inside* the production and stay. They are discouraged for interchange and
/// perfectly well-formed in XML, and a renderer that dropped them would be substituting on a rule
/// it made up.
///
/// The stand-in is [`char::REPLACEMENT_CHARACTER`], which is what the sanitizer already writes for
/// a C1 character that has no picture. One cell, like the value it replaces, so nothing placed
/// against the row moves — asserted in `a_scalar_xml_forbids_is_replaced_and_moves_nothing` rather
/// than assumed.
struct Representable<'a>(&'a mut dyn fmt::Write);

impl fmt::Write for Representable<'_> {
  /// In runs, like [`Escaped`]: ordinary text costs one `write_str` and only a forbidden scalar
  /// costs a call of its own.
  fn write_str(&mut self, text: &str) -> fmt::Result {
    let mut rest = text;
    while let Some(at) = rest.find(|character| !is_xml_char(character)) {
      let (before, from) = rest.split_at(at);
      self.0.write_str(before)?;
      let mut characters = from.chars();
      characters
        .next()
        .expect("`find` reported a character at this index");
      self.0.write_str("\u{fffd}")?;
      rest = characters.as_str();
    }
    self.0.write_str(rest)
  }
}

/// The caller's message, formatted once and held for the second pass — and refused before it can
/// be held unbounded.
///
/// # Why the message is the one input that needs a ceiling
///
/// [`Terminal::max_rendered_width`](super::Terminal::max_rendered_width) says painty does not bound
/// the caller's own words, because they **pass through**: printed once, straight to the writer, so
/// a writer unwilling to take them refuses them and the caller already spent the memory it is
/// asking painty to spend. Both halves of that are false in this renderer, and only in this one.
///
/// They do not pass through. An SVG declares its size in its root element, so the render runs
/// twice, and a [`fmt::Display`] may not be asked twice — so the message is formatted once and
/// **retained** across both passes. Nothing about it streams, and `out` is not consulted until all
/// of it is already in memory.
///
/// And it is not the caller's memory. Every other input a diagnostic carries is a `&str` the
/// caller allocated, so supplying a megabyte cost the caller a megabyte. The message is a
/// `&dyn Display` — two words that can synthesize a gigabyte, and the only input in the crate whose
/// size is not bounded by memory the caller has already spent.
///
/// # It refuses before it appends, which is the whole of the property
///
/// A ceiling tested against what has already been pushed has already paid for it: the bytes are in
/// the string by the time the test can see them, and the test is then a report rather than a bound.
/// So the length is checked against what is left **before** `push_str`, and a write that would
/// cross the ceiling appends nothing. Held bytes never exceed the ceiling, and what is allocated
/// follows what was actually written — an ordinary message costs what an ordinary message is,
/// because nothing here is reserved.
///
/// # A refusal has to survive a `Display` that ignores it
///
/// [`overflowed`](Self::overflowed) exists because the [`fmt::Error`] this returns is not enough on
/// its own. A `Display` implementation is free to discard the error its formatter hands back and
/// return `Ok`, and `core::fmt::write` reports what the `Display` returned — so a caller that
/// ignores errors would leave this holding a **truncated** message that the render then draws as
/// though it were whole. A diagnostic that silently drops the part its author wrote is the failure
/// this crate exists to prevent, so the refusal is recorded here and read back by
/// [`whole`](Self::whole) rather than trusted to propagate.
pub(super) struct Captured {
  text: String,
  /// How many more bytes may be appended.
  left: usize,
  /// Whether a write was ever refused — see the type's documentation.
  overflowed: bool,
}

impl Captured {
  pub(super) const fn new(ceiling: usize) -> Self {
    Self {
      text: String::new(),
      left: ceiling,
      overflowed: false,
    }
  }

  /// What was captured, or [`fmt::Error`] if any of it was refused.
  pub(super) fn whole(self) -> Result<String, fmt::Error> {
    if self.overflowed {
      return Err(fmt::Error);
    }
    Ok(self.text)
  }
}

impl fmt::Write for Captured {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    // Before the append and not after it — see the type's documentation.
    if text.len() > self.left {
      self.overflowed = true;
      return Err(fmt::Error);
    }
    self.left -= text.len();
    self.text.push_str(text);
    Ok(())
  }
}

/// A string as a [`fmt::Display`] that writes it in one call.
///
/// What [`Terminal::render_svg`](super::Terminal::render_svg) re-attaches its formatted message as,
/// and the reason is the seam under it. [`Shown`](super::paint::Shown) segments what each
/// `write_str` hands it, so a `Display` that split a grapheme cluster across two calls would be
/// measured as its parts — the defect this surface was just repaired for, arriving by a second
/// route. `<String as Display>` does not split anything, but that is a fact about the standard
/// library rather than about this crate, and six lines is cheaper than the assumption.
pub(super) struct Verbatim<'a>(pub(super) &'a str);

impl fmt::Display for Verbatim<'_> {
  #[inline]
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str(self.0)
  }
}

/// Whether `character` is one XML 1.0 permits — see [`Representable`].
const fn is_xml_char(character: char) -> bool {
  matches!(
    character,
    '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'
  )
}

/// The root element and the stylesheet, written from the caller's palette.
///
/// The size is the extent the first pass measured, plus a margin at each edge. `viewBox` as well as
/// `width` and `height`, because the first is what lets the image scale into a column of a README
/// and the second pair is what stops a viewer with no container from falling back to 300 by 150.
pub(super) fn open_document(
  out: &mut impl fmt::Write,
  cells: u64,
  rows: u64,
  palette: &dyn Palette,
) -> fmt::Result {
  let width = cells.saturating_mul(ADVANCE).saturating_add(PAD * 2);
  let height = LINE_HEIGHT.saturating_mul(rows).saturating_add(PAD * 2);
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
