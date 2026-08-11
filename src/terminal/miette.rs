//! A second presentation, written out in full before anything is abstracted.
//!
//! # Why this is a duplicate and not a parameter
//!
//! The design's Phase 2d says to land one style, implement a second, and let the trait be **what
//! actually differed** — because a trait designed against three prose descriptions is an
//! abstraction nobody consumed. So this file is the second style written against the same plan,
//! duplicating every row `Terminal` already writes, and the seam is cut afterwards from the
//! diff between the two.
//!
//! What is shared here is deliberately **not** presentation. [`Plan`](super::render::Plan) decides
//! which lines are drawn, which cells each end of each span occupies and which column a bracket
//! runs down; those are answers about the source and the caller's positions, and a style that
//! could change them would be a style that changes what the diagnostic is about. The palette and
//! the capability are shared for the same reason: colour is `Theme`'s, and these three styles
//! differ in glyphs and layout.
//!
//! # This is miette-SHAPED, and is not miette
//!
//! It follows the renderer the design names — a boxed location line, a box-drawing gutter, a
//! label hanging from its own row rather than sitting on the marker, and a multi-line span
//! bracketed down the margin instead of reaching into the source. It is not byte-compatible with
//! `miette` and does not try to be; what it is for is being **far enough from
//! [`Terminal`](super::Terminal)'s** to make the difference between them worth naming.

use core::fmt;

use super::{
  LineCells,
  render::{
    Connector, Ends, Frame, Input, Phrase, Plan, Terminal, column_of, digits, fill, never_empty,
    pad, slot, write_shown,
  },
  width::Row,
};
use crate::{Diagnostic, Palette, Role, Severity};

/// The four glyphs one bracket is drawn with, and the two an underline is.
///
/// A weight rather than a shape, because that is the axis left over. `Terminal` says which
/// position a diagnostic is ABOUT with a different character — `^` against `-` — and a box-drawing
/// style has no second character to spare: `─` is the underline. So the primary is drawn heavy and
/// a secondary light, which is legible with no colour at all and stays legible with it.
#[derive(Debug, Clone, Copy)]
struct Brackets {
  /// The corner on the source row a multi-line span opens on.
  opens: char,
  /// The bar it runs down.
  runs: char,
  /// The corner on the source row it closes on, which is a tee: the label hangs off it below.
  closes: char,
  /// The corner the label row starts with.
  corner: char,
  /// The horizontal run, used by an underline and by a label row's reach alike.
  rule: char,
  /// Where a single-line label hangs off its own underline.
  tee: char,
}

const HEAVY: Brackets = Brackets {
  opens: '┏',
  runs: '┃',
  closes: '┣',
  corner: '┗',
  rule: '━',
  tee: '┳',
};

const LIGHT: Brackets = Brackets {
  opens: '╭',
  runs: '│',
  closes: '├',
  corner: '╰',
  rule: '─',
  tee: '┬',
};

/// The glyphs a position of this importance is drawn with.
const fn brackets(primary: bool) -> Brackets {
  if primary { HEAVY } else { LIGHT }
}

/// The character that announces a severity, where `Terminal` writes the word.
const fn glyph(severity: Severity) -> char {
  match severity {
    Severity::Error => '×',
    Severity::Warning => '⚠',
    Severity::Advice => '☞',
  }
}

/// What stands in a bracket's column on the source row of `line`.
///
/// Three answers where [`Connector::on_source_row`] has two, and that is the divergence rather
/// than a refinement: this style says where a span CLOSES on the source row itself, so the row
/// under it carries only the label. `Terminal` says it on a row of its own that reaches back into
/// the source, and so has nothing to put here but the running bar.
const fn on_source_row(connector: &Connector, line: u64) -> Option<char> {
  let glyphs = brackets(matches!(connector.role, Role::PrimaryLabel));
  if line < connector.first || line > connector.last {
    None
  } else if line == connector.first {
    Some(glyphs.opens)
  } else if line == connector.last {
    Some(glyphs.closes)
  } else {
    Some(glyphs.runs)
  }
}

impl<P: Palette> Terminal<P> {
  /// Writes `diagnostic` in the second style.
  ///
  /// Not public, and not yet a choice a caller can make: this exists to be the second
  /// implementation the trait is derived from, and publishing a selector before the seam is cut
  /// would freeze the seam at whatever this file happened to need first.
  pub(super) fn render_boxed(
    &self,
    diagnostic: &Diagnostic<'_>,
    inputs: &[Input<'_>],
    out: &mut impl fmt::Write,
  ) -> fmt::Result {
    // The code above the message rather than inside it, and a glyph where the word was. Both are
    // this style's, and the second is the one that costs something: `error` is a word a reader
    // can search for and `×` is not, which is the trade a boxed style makes for a narrower header.
    let severity = diagnostic.severity();
    self.styled(out, Role::Code, diagnostic.code())?;
    out.write_str("\n\n  ")?;
    self.styled(
      out,
      Role::Severity(severity),
      glyph(severity).encode_utf8(&mut [0; 4]),
    )?;
    out.write_char(' ')?;
    write_shown(out, diagnostic.message())?;
    out.write_char('\n')?;

    let mut plan = Plan::of(diagnostic, inputs, self.measure());
    let measure = self.measure();
    let gutter = plan.gutter();

    let Plan {
      blocks,
      excerpts,
      marks,
      connectors,
    } = &mut plan;
    let mut margin: Vec<Option<(char, Role)>> = Vec::new();

    for block in blocks.iter() {
      // The location line and the rule that opens the block are ONE row here, where `Terminal`
      // writes two. That is not a saving: the box has a top, the top has to say something, and
      // where the excerpt came from is what it says.
      pad(out, gutter + 2)?;
      self.styled(out, Role::Gutter, "╭─[")?;
      if let Some(origin) = block.origin {
        write_shown(out, origin)?;
        out.write_char(':')?;
      }
      write!(out, "{}:{}", block.line, block.column)?;
      self.styled(out, Role::Gutter, "]")?;
      out.write_char('\n')?;

      let running = &connectors[block.connectors.clone()];
      margin.clear();
      margin.resize(slot(block.depth), None);

      for index in block.excerpts.clone() {
        let excerpt = &excerpts[index];
        let number = excerpt.line.number();
        let cells = measure.cells(excerpt.line);
        let placed = &mut marks[excerpt.marks.clone()];
        let row = cells.place_marks(placed, measure.budget);

        if let Some(above) = excerpt.after {
          fill(&mut margin, running, |connector| {
            connector
              .spans_the_gap(above, number)
              .then(|| brackets(matches!(connector.role, Role::PrimaryLabel)).runs)
          });
          self.boxed_elision_row(out, gutter, &margin)?;
        }

        fill(&mut margin, running, |connector| {
          on_source_row(connector, number)
        });
        self.boxed_source_row(out, gutter, &margin, &cells, &row)?;

        for mark in placed.iter() {
          let phrase = *mark.payload();
          let columns = never_empty(mark.columns());
          match phrase.ends {
            Ends::Whole => self.boxed_whole(out, gutter, &margin, columns, phrase)?,
            // Nothing. Where a span opens is said by the corner already drawn in the margin of
            // the source row above, so there is no row here and no cell marked — which is the
            // whole of what this style gives up in exchange for never needing one.
            Ends::Opens => {
              if let Some(column) = column_of(&mut margin, phrase.depth) {
                *column = Some((brackets(phrase.primary).runs, phrase.role()));
              }
            }
            Ends::Closes => {
              let frame = Frame::new(gutter, block.depth, &margin);
              self.boxed_closes(out, frame, phrase)?;
              if let Some(column) = column_of(&mut margin, phrase.depth) {
                *column = None;
              }
            }
          }
        }
      }

      pad(out, gutter + 2)?;
      self.styled(out, Role::Gutter, "╰────")?;
      out.write_char('\n')?;
    }

    if let Some(help) = diagnostic.help() {
      out.write_str("  ")?;
      self.styled(out, Role::Help, "help")?;
      out.write_str(": ")?;
      self.styled(out, Role::Help, help)?;
      out.write_char('\n')?;
    }
    Ok(())
  }

  /// ` NN │ ` — the number, right-aligned, and the box's own wall.
  ///
  /// One cell wider than `Terminal`'s on the left, because the wall is the box's and the box is
  /// inset from the header above it.
  fn boxed_source_field(&self, out: &mut impl fmt::Write, gutter: u64, number: u64) -> fmt::Result {
    out.write_char(' ')?;
    pad(out, gutter - digits(number))?;
    self.styled(out, Role::LineNumber, &number.to_string())?;
    out.write_char(' ')?;
    self.styled(out, Role::Gutter, "│")?;
    out.write_char(' ')
  }

  /// `   · ` — the field of a row that says something about the line above it.
  ///
  /// The dots are what says the row is not a line of the file. `Terminal` leaves the number field
  /// blank and repeats the bar; here the wall changes character instead, so a reader scanning the
  /// left edge sees where the source stops without counting spaces.
  fn boxed_annotation_field(&self, out: &mut impl fmt::Write, gutter: u64) -> fmt::Result {
    pad(out, gutter + 2)?;
    self.styled(out, Role::Gutter, "·")?;
    out.write_char(' ')
  }

  /// One source line, written once.
  fn boxed_source_row(
    &self,
    out: &mut impl fmt::Write,
    gutter: u64,
    margin: &[Option<(char, Role)>],
    cells: &LineCells<'_>,
    row: &Row,
  ) -> fmt::Result {
    self.boxed_source_field(out, gutter, cells.line().number())?;
    self.margin(out, margin)?;
    self.styled_with(out, Role::SourceText, |shown| {
      cells.write_expanded_upto(shown, row.drawn_end)?;
      if row.elided {
        fmt::Write::write_char(shown, '…')?;
      }
      Ok(())
    })?;
    out.write_char('\n')
  }

  /// The row standing for the lines left out.
  ///
  /// In the WALL's column rather than the number's, which is the other way round from
  /// `Terminal`'s `...`. Both are true and they say different things: `...` stands where the
  /// numbers would be and reads as "numbers are missing", `⋮` stands in the wall and reads as
  /// "rows are missing". A boxed style has a wall to say it in.
  fn boxed_elision_row(
    &self,
    out: &mut impl fmt::Write,
    gutter: u64,
    margin: &[Option<(char, Role)>],
  ) -> fmt::Result {
    pad(out, gutter + 2)?;
    self.styled(out, Role::LineNumber, "⋮")?;
    let Some(last) = margin.iter().rposition(Option::is_some) else {
      return out.write_char('\n');
    };
    out.write_char(' ')?;
    self.margin_upto(out, &margin[..=last])?;
    out.write_char('\n')
  }

  /// A span drawn whole on one line: an underline, and the label hanging off it below.
  ///
  /// TWO rows where `Terminal` writes one, and that is the divergence rather than a taste. A
  /// marker row that also carries its label ends at whatever the label's length is, so a second
  /// label further right on the same line has nowhere to go; hanging the label off a tee leaves
  /// the underline row free for every span on the line. painty does not merge them yet — that is
  /// the row-assignment problem `Terminal` names as the next one — and this is the shape that
  /// will not have to change when it does.
  fn boxed_whole(
    &self,
    out: &mut impl fmt::Write,
    gutter: u64,
    margin: &[Option<(char, Role)>],
    marks: core::ops::Range<u64>,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    let glyphs = brackets(phrase.primary);
    let role = phrase.role();
    let width = marks.end - marks.start;
    // Where the label hangs from, and none when there is nothing to hang: a tee with no row under
    // it is a promise the render does not keep.
    let tee = phrase.text.map(|_| width / 2);

    self.boxed_annotation_field(out, gutter)?;
    self.margin(out, margin)?;
    pad(out, marks.start - 1)?;
    self.styled_with(out, role, |shown| {
      for cell in 0..width {
        let at = if Some(cell) == tee {
          glyphs.tee
        } else {
          glyphs.rule
        };
        fmt::Write::write_char(shown, at)?;
      }
      Ok(())
    })?;
    out.write_char('\n')?;

    let (Some(text), Some(hangs)) = (phrase.text, tee) else {
      return Ok(());
    };
    self.boxed_annotation_field(out, gutter)?;
    self.margin(out, margin)?;
    pad(out, marks.start - 1 + hangs)?;
    self.styled_with(out, role, |shown| {
      fmt::Write::write_char(shown, glyphs.corner)?;
      fmt::Write::write_char(shown, glyphs.rule)?;
      fmt::Write::write_char(shown, glyphs.rule)
    })?;
    out.write_char(' ')?;
    self.styled(out, role, text)?;
    out.write_char('\n')
  }

  /// Where a multi-line span closes, and where its label is said.
  ///
  /// It never leaves the margin. `Terminal`'s closing runs back along the row to the cell the
  /// span's last character sits in and puts a marker there; this one turns the corner under the
  /// bracket and says the label, so the closing END of a multi-line span is marked on no cell at
  /// all. That is the cost of the form and it is what
  /// `which_cells_are_marked_is_the_same_in_both_styles` has to be stated around: the two styles
  /// point at the same LINES, and only one of them points at a column.
  ///
  /// The run still crosses every column to the right, for the reason `Terminal`'s underscores do:
  /// a span still open out there opened later, so the crossing says this bracket closes over it.
  fn boxed_closes(
    &self,
    out: &mut impl fmt::Write,
    frame: Frame<'_>,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    let Frame {
      gutter,
      depth,
      margin,
    } = frame;
    let glyphs = brackets(phrase.primary);
    let role = phrase.role();
    self.boxed_annotation_field(out, gutter)?;
    let own = slot(phrase.depth).saturating_sub(1).min(margin.len());
    self.margin_upto(out, &margin[..own])?;

    let reach = depth.saturating_sub(phrase.depth) + 4;
    self.styled_with(out, role, |shown| {
      fmt::Write::write_char(shown, glyphs.corner)?;
      for _ in 0..reach {
        fmt::Write::write_char(shown, glyphs.rule)?;
      }
      Ok(())
    })?;
    if let Some(text) = phrase.text {
      out.write_char(' ')?;
      self.styled(out, role, text)?;
    }
    out.write_char('\n')
  }
}
