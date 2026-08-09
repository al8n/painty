use super::{Ansi16, Color, Palette, Role, Style, Theme};
use crate::Severity;

/// Every role a theme has to answer for.
///
/// Written out rather than derived, so that adding a role to `Role` does not silently leave it
/// untested — `Theme`'s own matches are exhaustive in-crate, so the compiler already forces a
/// decision there, and this forces the decision to be exercised.
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

#[test]
fn the_sixteen_round_trip_through_their_indices() {
  for index in 0u8..16 {
    let colour = Ansi16::from_index(index).expect("an index below sixteen names a colour");
    assert_eq!(colour.index(), index);
  }
  for index in 16u8..=255 {
    assert!(
      Ansi16::from_index(index).is_none(),
      "index {index} is not one of the sixteen"
    );
  }
}

#[test]
fn narrowing_to_the_sixteen_is_idempotent_and_total() {
  // Total over the whole 256 palette, and over a stride across the colour cube. The stride is 17
  // because it is coprime with the cube's spacing, so it does not land on the same quantisation
  // boundary in every channel.
  for index in 0u8..=255 {
    let once = Color::Ansi256(index).to_ansi16();
    assert_eq!(once.to_ansi16(), once, "Ansi256({index})");
    assert!(matches!(once, Color::Ansi16(_)));
  }
  let mut red = 0u16;
  while red <= 255 {
    let mut green = 0u16;
    while green <= 255 {
      let mut blue = 0u16;
      while blue <= 255 {
        let colour = Color::Rgb(red as u8, green as u8, blue as u8);
        let once = colour.to_ansi16();
        assert_eq!(once.to_ansi16(), once, "{colour:?}");
        assert!(matches!(once, Color::Ansi16(_)), "{colour:?}");
        blue += 17;
      }
      green += 17;
    }
    red += 17;
  }
}

#[test]
fn narrowing_to_the_palette_is_idempotent_and_leaves_the_narrow_alone() {
  for index in 0u8..=255 {
    let colour = Color::Ansi256(index);
    assert_eq!(colour.to_ansi256(), colour, "already narrow");
  }
  for index in 0u8..16 {
    let colour = Color::Ansi16(Ansi16::from_index(index).expect("below sixteen"));
    assert_eq!(colour.to_ansi256(), colour, "already narrower");
  }
  let mut channel = 0u16;
  while channel <= 255 {
    let colour = Color::Rgb(channel as u8, 0, 128);
    let once = colour.to_ansi256();
    assert_eq!(once.to_ansi256(), once, "{colour:?}");
    channel += 17;
  }
}

#[test]
fn a_grey_takes_the_grey_ramp_rather_than_the_cube() {
  // The ramp resolves grey in 24 steps where the cube has six levels, so a mid grey must not be
  // rounded onto a cube corner. 0x80 is the case a naive quantiser gets visibly wrong.
  assert_eq!(Color::Rgb(128, 128, 128).to_ansi256(), Color::Ansi256(244));
  assert_eq!(Color::Rgb(8, 8, 8).to_ansi256(), Color::Ansi256(232));
  // Below and above the ramp there is nothing but the cube's own black and white corners.
  assert_eq!(Color::Rgb(0, 0, 0).to_ansi256(), Color::Ansi256(16));
  assert_eq!(Color::Rgb(255, 255, 255).to_ansi256(), Color::Ansi256(231));
}

#[test]
fn the_primaries_narrow_to_themselves() {
  // A downgrade that is merely total is not enough; it has to be recognisable. These are the cases
  // a reader would notice immediately.
  assert_eq!(
    Color::Rgb(255, 0, 0).to_ansi16(),
    Color::Ansi16(Ansi16::BrightRed)
  );
  assert_eq!(
    Color::Rgb(0, 255, 0).to_ansi16(),
    Color::Ansi16(Ansi16::BrightGreen)
  );
  assert_eq!(
    Color::Rgb(0, 0, 255).to_ansi16(),
    Color::Ansi16(Ansi16::BrightBlue)
  );
  assert_eq!(
    Color::Rgb(255, 255, 0).to_ansi16(),
    Color::Ansi16(Ansi16::BrightYellow)
  );
  assert_eq!(
    Color::Rgb(0, 0, 0).to_ansi16(),
    Color::Ansi16(Ansi16::Black)
  );
  assert_eq!(
    Color::Rgb(255, 255, 255).to_ansi16(),
    Color::Ansi16(Ansi16::BrightWhite)
  );
}

#[test]
fn a_style_reports_what_was_asked_of_it() {
  let plain = Style::plain();
  assert!(plain.is_plain());
  assert_eq!(plain.foreground(), None);
  assert!(!plain.bold() && !plain.italic() && !plain.underline());

  let dressed = Style::plain()
    .with_foreground(Color::Ansi16(Ansi16::Red))
    .with_background(Color::Rgb(1, 2, 3))
    .with_bold()
    .with_italic()
    .with_underline();
  assert!(!dressed.is_plain());
  assert_eq!(dressed.foreground(), Some(Color::Ansi16(Ansi16::Red)));
  assert_eq!(dressed.background(), Some(Color::Rgb(1, 2, 3)));
  assert!(dressed.bold() && dressed.italic() && dressed.underline());
}

#[test]
fn dropping_colour_keeps_every_attribute() {
  // The two-colour terminal's answer. Losing bold along with the colour would leave a header
  // indistinguishable from its own source line.
  let dressed = Style::plain()
    .with_foreground(Color::Rgb(9, 9, 9))
    .with_background(Color::Ansi256(4))
    .with_bold()
    .with_underline();
  let bare = dressed.without_color();

  assert_eq!(bare.foreground(), None);
  assert_eq!(bare.background(), None);
  assert!(bare.bold() && bare.underline());
  assert!(!bare.is_plain(), "attributes still make it non-plain");
}

#[test]
fn narrowing_a_style_narrows_both_colours_and_touches_nothing_else() {
  let wide = Style::plain()
    .with_foreground(Color::Rgb(255, 0, 0))
    .with_background(Color::Rgb(0, 0, 255))
    .with_italic();
  let narrow = wide.to_ansi16();

  assert_eq!(narrow.foreground(), Some(Color::Ansi16(Ansi16::BrightRed)));
  assert_eq!(narrow.background(), Some(Color::Ansi16(Ansi16::BrightBlue)));
  assert!(narrow.italic());
  assert_eq!(narrow.to_ansi16(), narrow, "idempotent");
}

#[test]
fn setting_a_role_is_what_reading_it_back_returns() {
  // The `with` match and the `style` match are two lists of the same ten fields, which is exactly
  // the shape a copy-paste puts the wrong field in. Every role is set to a value only it could
  // have, and read back.
  for (index, role) in ROLES.iter().enumerate() {
    let marker = Style::plain().with_foreground(Color::Ansi256(index as u8));
    let theme = Theme::new().with(*role, marker);
    assert_eq!(theme.style(*role), marker, "{role:?} did not round-trip");

    // ...and no other role moved with it.
    for other in &ROLES {
      if other != role {
        assert_eq!(
          theme.style(*other),
          Theme::new().style(*other),
          "setting {role:?} disturbed {other:?}"
        );
      }
    }
  }
}

#[test]
fn monochrome_asks_for_no_colour_anywhere() {
  let theme = Theme::monochrome();
  for role in ROLES {
    let style = theme.style(role);
    assert_eq!(style.foreground(), None, "{role:?}");
    assert_eq!(style.background(), None, "{role:?}");
  }
  // And it is still not uniformly plain, or there would be nothing to read.
  assert!(theme.style(Role::Severity(Severity::Error)).bold());
}

#[test]
fn every_built_in_theme_answers_every_role() {
  // A theme with a hole would render some part of a diagnostic in whatever the terminal happened
  // to be using, which reads as a bug in the caller's program rather than in painty.
  for theme in [Theme::new(), Theme::high_contrast(), Theme::monochrome()] {
    for role in ROLES {
      let _ = theme.style(role);
    }
  }
  // High contrast must differ from the default somewhere, or it is a name with nothing behind it.
  assert_ne!(Theme::high_contrast(), Theme::new());
  assert_ne!(Theme::monochrome(), Theme::new());
}

#[test]
fn a_palette_behind_a_reference_is_a_palette() {
  // A renderer takes `impl Palette`, and a caller holding a `&Theme` should not have to clone it.
  fn ask(palette: impl Palette) -> Style {
    palette.style(Role::Help)
  }
  let theme = Theme::new();
  // Through a reference explicitly, since that blanket impl is the thing under test — clippy would
  // otherwise have this pass `Theme` directly and exercise the inherent impl instead.
  let by_reference: &Theme = &theme;
  assert_eq!(ask(by_reference), theme.style(Role::Help));
}
