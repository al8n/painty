/// One of the sixteen colours every terminal has had since before it was a terminal.
///
/// The floor a downgrade lands on. A theme written in truecolour still has to be legible on a
/// sixteen-colour terminal, so [`Color::to_ansi16`] exists and this is what it produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ansi16 {
  /// Colour 0.
  Black,
  /// Colour 1.
  Red,
  /// Colour 2.
  Green,
  /// Colour 3.
  Yellow,
  /// Colour 4.
  Blue,
  /// Colour 5.
  Magenta,
  /// Colour 6.
  Cyan,
  /// Colour 7.
  White,
  /// Colour 8.
  BrightBlack,
  /// Colour 9.
  BrightRed,
  /// Colour 10.
  BrightGreen,
  /// Colour 11.
  BrightYellow,
  /// Colour 12.
  BrightBlue,
  /// Colour 13.
  BrightMagenta,
  /// Colour 14.
  BrightCyan,
  /// Colour 15.
  BrightWhite,
}

impl Ansi16 {
  /// Returns the colour's index, `0` to `15`.
  ///
  /// `u8` by rule 4 of [the numeric widths](crate#numeric-widths): this is the number that goes
  /// into an SGR escape sequence, and the sequence fixes its width.
  #[inline]
  pub const fn index(&self) -> u8 {
    match self {
      Self::Black => 0,
      Self::Red => 1,
      Self::Green => 2,
      Self::Yellow => 3,
      Self::Blue => 4,
      Self::Magenta => 5,
      Self::Cyan => 6,
      Self::White => 7,
      Self::BrightBlack => 8,
      Self::BrightRed => 9,
      Self::BrightGreen => 10,
      Self::BrightYellow => 11,
      Self::BrightBlue => 12,
      Self::BrightMagenta => 13,
      Self::BrightCyan => 14,
      Self::BrightWhite => 15,
    }
  }

  /// The colour at `index`, or `None` past fifteen.
  #[inline]
  pub const fn from_index(index: u8) -> Option<Self> {
    Some(match index {
      0 => Self::Black,
      1 => Self::Red,
      2 => Self::Green,
      3 => Self::Yellow,
      4 => Self::Blue,
      5 => Self::Magenta,
      6 => Self::Cyan,
      7 => Self::White,
      8 => Self::BrightBlack,
      9 => Self::BrightRed,
      10 => Self::BrightGreen,
      11 => Self::BrightYellow,
      12 => Self::BrightBlue,
      13 => Self::BrightMagenta,
      14 => Self::BrightCyan,
      15 => Self::BrightWhite,
      _ => return None,
    })
  }

  /// The colour's nominal sRGB value, as terminals conventionally render it.
  ///
  /// Used to pick a nearest neighbour when narrowing, and offered publicly because a renderer for
  /// a medium that has no palette — HTML, a native widget — needs *some* concrete value for a
  /// theme written in these terms.
  #[inline]
  pub const fn to_rgb(&self) -> (u8, u8, u8) {
    match self {
      Self::Black => (0, 0, 0),
      Self::Red => (128, 0, 0),
      Self::Green => (0, 128, 0),
      Self::Yellow => (128, 128, 0),
      Self::Blue => (0, 0, 128),
      Self::Magenta => (128, 0, 128),
      Self::Cyan => (0, 128, 128),
      Self::White => (192, 192, 192),
      Self::BrightBlack => (128, 128, 128),
      Self::BrightRed => (255, 0, 0),
      Self::BrightGreen => (0, 255, 0),
      Self::BrightYellow => (255, 255, 0),
      Self::BrightBlue => (0, 0, 255),
      Self::BrightMagenta => (255, 0, 255),
      Self::BrightCyan => (0, 255, 255),
      Self::BrightWhite => (255, 255, 255),
    }
  }
}

/// A colour, in whichever of the three terminal representations a theme chose.
///
/// # Why this is a concrete enum and [`Palette`](super::Palette) is a trait
///
/// The instinct is to make colour a trait so a consumer is not boxed in by a closed set. That
/// instinct is right about *where* to be open and wrong here, for two reasons that are not
/// stylistic.
///
/// **Downgrading requires seeing inside.** A terminal renderer must map `Rgb → 256 → 16` for
/// whatever the terminal turned out to support. If a colour were opaque, painty could only ask an
/// implementor for its ANSI form, and every theme author would write that narrowing again,
/// differently. [`to_ansi256`](Self::to_ansi256) and [`to_ansi16`](Self::to_ansi16) are why the
/// variants are visible.
///
/// **A model export has to be plain data.** A trait object does not cross a C ABI; a value the
/// far side can lay out in a struct does.
///
/// `#[non_exhaustive]` gives the openness that is actually wanted, in the other direction: painty
/// can add a representation — a wide gamut, say — without breaking a consumer, while keeping the
/// ability to convert between them. The openness worth having is one level up, and it is free:
/// [`Palette`](super::Palette) is a trait, so a consumer can compute styles however it likes.
///
/// ```
/// use painty::{Ansi16, Color};
///
/// // A truecolour theme still has to be legible on a sixteen-colour terminal.
/// assert_eq!(Color::Rgb(255, 0, 0).to_ansi16(), Color::Ansi16(Ansi16::BrightRed));
/// // Narrowing is idempotent: what is already narrow stays put.
/// let narrow = Color::Rgb(12, 200, 30).to_ansi16();
/// assert_eq!(narrow.to_ansi16(), narrow);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Color {
  /// One of the sixteen the terminal has always had.
  Ansi16(Ansi16),
  /// An index into the terminal's 256-colour palette.
  ///
  /// `u8` by rule 4 of [the numeric widths](crate#numeric-widths): the SGR escape sequence painty
  /// emits fixes this at one byte, and a value outside it is not expressible in the format.
  Ansi256(u8),
  /// A 24-bit colour, as `(red, green, blue)`.
  ///
  /// `u8` per channel by rule 4, for the same reason.
  Rgb(u8, u8, u8),
}

impl Color {
  /// Narrows to the 256-colour palette, leaving anything already at or below it alone.
  ///
  /// The cube first: 216 of the 256 entries are a 6×6×6 grid, and 24 more are a grey ramp. A
  /// colour whose channels agree is put on the ramp, which is finer there than the cube is, and
  /// everything else is quantised into the cube.
  #[inline]
  pub const fn to_ansi256(self) -> Self {
    match self {
      Self::Rgb(red, green, blue) => Self::Ansi256(rgb_to_ansi256(red, green, blue)),
      other => other,
    }
  }

  /// Narrows all the way to the sixteen, whatever the starting representation.
  ///
  /// Idempotent, total, and the honest fallback for a terminal that can do no better. Downgrade
  /// rather than refuse: a truecolour theme on a sixteen-colour terminal should look worse, not
  /// disappear.
  #[inline]
  pub const fn to_ansi16(self) -> Self {
    match self {
      Self::Ansi16(_) => self,
      Self::Ansi256(index) => Self::Ansi16(ansi256_to_ansi16(index)),
      Self::Rgb(red, green, blue) => {
        Self::Ansi16(ansi256_to_ansi16(rgb_to_ansi256(red, green, blue)))
      }
    }
  }
}

/// The 6×6×6 cube's level for one channel.
#[inline]
const fn cube_level(channel: u8) -> u8 {
  // The cube's levels are 0, 95, 135, 175, 215, 255 — the first gap is much wider than the rest,
  // so the midpoints have to be spelled out rather than divided.
  if channel < 48 {
    0
  } else if channel < 115 {
    1
  } else if channel < 155 {
    2
  } else if channel < 195 {
    3
  } else if channel < 235 {
    4
  } else {
    5
  }
}

#[inline]
const fn rgb_to_ansi256(red: u8, green: u8, blue: u8) -> u8 {
  if red == green && green == blue {
    // The grey ramp is 24 steps of 10 from 8 to 238, and it resolves grey far more finely than the
    // cube's six levels do.
    if red < 8 {
      return 16;
    }
    if red > 238 {
      return 231;
    }
    return 232 + (red - 8) / 10;
  }
  16 + 36 * cube_level(red) + 6 * cube_level(green) + cube_level(blue)
}

#[inline]
const fn ansi256_to_ansi16(index: u8) -> Ansi16 {
  if index < 16 {
    // `from_index` is total below sixteen; the arm is unreachable and `Black` is the inert answer.
    return match Ansi16::from_index(index) {
      Some(colour) => colour,
      None => Ansi16::Black,
    };
  }
  if index >= 232 {
    // The grey ramp, split across the four greys the sixteen offer.
    let step = index - 232;
    return if step < 6 {
      Ansi16::Black
    } else if step < 13 {
      Ansi16::BrightBlack
    } else if step < 19 {
      Ansi16::White
    } else {
      Ansi16::BrightWhite
    };
  }

  // Back out of the cube, then pick the nearest of the sixteen by which channels are lit.
  let cube = index - 16;
  let red = cube / 36;
  let green = (cube % 36) / 6;
  let blue = cube % 6;
  let bright = red > 3 || green > 3 || blue > 3;

  match (red >= 3, green >= 3, blue >= 3) {
    (false, false, false) => {
      if bright {
        Ansi16::BrightBlack
      } else {
        Ansi16::Black
      }
    }
    (true, false, false) => {
      if bright {
        Ansi16::BrightRed
      } else {
        Ansi16::Red
      }
    }
    (false, true, false) => {
      if bright {
        Ansi16::BrightGreen
      } else {
        Ansi16::Green
      }
    }
    (false, false, true) => {
      if bright {
        Ansi16::BrightBlue
      } else {
        Ansi16::Blue
      }
    }
    (true, true, false) => {
      if bright {
        Ansi16::BrightYellow
      } else {
        Ansi16::Yellow
      }
    }
    (true, false, true) => {
      if bright {
        Ansi16::BrightMagenta
      } else {
        Ansi16::Magenta
      }
    }
    (false, true, true) => {
      if bright {
        Ansi16::BrightCyan
      } else {
        Ansi16::Cyan
      }
    }
    (true, true, true) => {
      if bright {
        Ansi16::BrightWhite
      } else {
        Ansi16::White
      }
    }
  }
}
