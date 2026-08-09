/// What a caller wants, before the environment gets a say.
///
/// The three-state shape every command-line tool already speaks, so a `--color` flag maps onto it
/// without a conversion layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorChoice {
  /// Let the environment decide. See [`ColorChoice::resolve`] for the order it is asked in.
  #[default]
  Auto,
  /// Colour, whatever the environment says.
  ///
  /// For the caller who knows better than the heuristic: writing to a file destined for `less -R`,
  /// or to a CI log that renders escapes.
  Always,
  /// No colour, whatever the environment says.
  Never,
}

/// How much colour the output can actually carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColorCapability {
  /// None at all. Attributes still work — see [`Style::without_color`](crate::Style::without_color).
  None,
  /// The sixteen.
  Ansi16,
  /// The 256-colour palette.
  Ansi256,
  /// Twenty-four bits.
  TrueColor,
}

/// Everything the decision reads, captured.
///
/// # Why the inputs are a value and not the process
///
/// Whether a terminal supports colour is an environment question, and an environment is the one
/// thing a test cannot have an oracle for. So the decision is a pure function of this snapshot,
/// and reading the real process is a separate, tiny step ([`Environment::from_process`]). That
/// makes the *procedure* exhaustively testable — every combination of every input, against the
/// order written down — while the part that cannot be tested is the part with no logic in it.
///
/// A borrowed view, so capturing it allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Environment<'a> {
  no_color: Option<&'a str>,
  clicolor_force: Option<&'a str>,
  clicolor: Option<&'a str>,
  term: Option<&'a str>,
  colorterm: Option<&'a str>,
  is_terminal: bool,
}

impl<'a> Environment<'a> {
  /// An environment with nothing set and no terminal attached.
  #[inline]
  #[must_use]
  pub const fn new() -> Self {
    Self {
      no_color: None,
      clicolor_force: None,
      clicolor: None,
      term: None,
      colorterm: None,
      is_terminal: false,
    }
  }

  /// Sets `NO_COLOR`.
  #[inline]
  #[must_use]
  pub const fn with_no_color(mut self, value: Option<&'a str>) -> Self {
    self.no_color = value;
    self
  }

  /// Sets `CLICOLOR_FORCE`.
  #[inline]
  #[must_use]
  pub const fn with_clicolor_force(mut self, value: Option<&'a str>) -> Self {
    self.clicolor_force = value;
    self
  }

  /// Sets `CLICOLOR`.
  #[inline]
  #[must_use]
  pub const fn with_clicolor(mut self, value: Option<&'a str>) -> Self {
    self.clicolor = value;
    self
  }

  /// Sets `TERM`.
  #[inline]
  #[must_use]
  pub const fn with_term(mut self, value: Option<&'a str>) -> Self {
    self.term = value;
    self
  }

  /// Sets `COLORTERM`.
  #[inline]
  #[must_use]
  pub const fn with_colorterm(mut self, value: Option<&'a str>) -> Self {
    self.colorterm = value;
    self
  }

  /// Sets whether the stream being written to is a terminal.
  #[inline]
  #[must_use]
  pub const fn with_terminal(mut self, is_terminal: bool) -> Self {
    self.is_terminal = is_terminal;
    self
  }

  /// Reads the four variables and asks `stream` whether it is a terminal.
  ///
  /// The whole of painty's relationship with the process. It has no logic in it on purpose: every
  /// decision made from these values is in [`ColorChoice::resolve`], where it can be tested.
  #[must_use]
  pub fn from_process(
    stream: &impl std::io::IsTerminal,
    captured: &'a CapturedEnvironment,
  ) -> Self {
    Self {
      no_color: captured.no_color.as_deref(),
      clicolor_force: captured.clicolor_force.as_deref(),
      clicolor: captured.clicolor.as_deref(),
      term: captured.term.as_deref(),
      colorterm: captured.colorterm.as_deref(),
      is_terminal: stream.is_terminal(),
    }
  }

  /// The colour level this environment can carry, ignoring whether colour is wanted at all.
  ///
  /// Separated from the gate below because the two answer different questions and confusing them
  /// is how `CLICOLOR_FORCE` ends up producing monochrome on a 256-colour terminal.
  fn level(&self) -> ColorCapability {
    if matches!(self.colorterm, Some("truecolor" | "24bit")) {
      return ColorCapability::TrueColor;
    }
    match self.term {
      Some(term) if term.contains("256color") => ColorCapability::Ansi256,
      _ => ColorCapability::Ansi16,
    }
  }

  /// Whether colour is wanted at all, under [`ColorChoice::Auto`].
  ///
  /// The order is the whole of the decision, and each step is only reached because every step
  /// above it declined to answer:
  ///
  /// 1. `NO_COLOR` set to anything non-empty turns colour off. Presence, not truthiness — the
  ///    convention is explicit that `NO_COLOR=0` still means no colour.
  /// 2. `CLICOLOR_FORCE` non-empty and not `0` turns it on **even when the stream is not a
  ///    terminal**, which is the whole reason the variable exists and the reason it is above the
  ///    terminal test rather than below it.
  /// 3. A stream that is not a terminal gets none.
  /// 4. `CLICOLOR=0`, or a `TERM` of `dumb`, turns it off.
  /// 5. Otherwise, colour.
  fn wanted(&self) -> bool {
    if self.no_color.is_some_and(|value| !value.is_empty()) {
      return false;
    }
    if self
      .clicolor_force
      .is_some_and(|value| !value.is_empty() && value != "0")
    {
      return true;
    }
    if !self.is_terminal {
      return false;
    }
    if self.clicolor == Some("0") || self.term == Some("dumb") {
      return false;
    }
    true
  }
}

/// The environment variables, owned, so a borrowed [`Environment`] can point into them.
///
/// Reading a variable allocates; the decision does not. Splitting them keeps the allocation at the
/// edge, where a caller can hoist it out of a loop over many diagnostics.
#[derive(Debug, Clone, Default)]
pub struct CapturedEnvironment {
  no_color: Option<String>,
  clicolor_force: Option<String>,
  clicolor: Option<String>,
  term: Option<String>,
  colorterm: Option<String>,
}

impl CapturedEnvironment {
  /// Reads `NO_COLOR`, `CLICOLOR_FORCE`, `CLICOLOR`, `TERM` and `COLORTERM`.
  #[must_use]
  pub fn from_process() -> Self {
    let read = |name: &str| std::env::var(name).ok();
    Self {
      no_color: read("NO_COLOR"),
      clicolor_force: read("CLICOLOR_FORCE"),
      clicolor: read("CLICOLOR"),
      term: read("TERM"),
      colorterm: read("COLORTERM"),
    }
  }
}

impl ColorChoice {
  /// Resolves what to actually emit.
  ///
  /// # The precedence, top to bottom
  ///
  /// 1. **The caller.** [`Never`](Self::Never) is none and [`Always`](Self::Always) is colour, and
  ///    neither consults the environment about *whether* — only about *how much*. A caller that
  ///    has decided is not overruled by a heuristic.
  /// 2. **`NO_COLOR`**, then **`CLICOLOR_FORCE`**, then **the stream**, then **`CLICOLOR` and
  ///    `TERM=dumb`**. Each step is reached only because every step above it declined to answer:
  ///    `NO_COLOR` is presence rather than truthiness, so `NO_COLOR=0` is still none;
  ///    `CLICOLOR_FORCE` sits above the terminal test because turning colour on for a pipe is the
  ///    whole reason it exists; and a stream that is not a terminal is already off before
  ///    `CLICOLOR` or `TERM` are consulted.
  ///
  /// The level — sixteen, 256 or truecolour — is read from `COLORTERM` and `TERM` in every case
  /// where colour is emitted at all, including when it was forced, and never falls below
  /// [`Ansi16`](ColorCapability::Ansi16). Forcing colour onto a pipe and getting monochrome would
  /// be a strange reward for asking.
  #[must_use]
  pub fn resolve(self, environment: &Environment<'_>) -> ColorCapability {
    match self {
      Self::Never => ColorCapability::None,
      Self::Always => environment.level(),
      Self::Auto => {
        if environment.wanted() {
          environment.level()
        } else {
          ColorCapability::None
        }
      }
    }
  }
}
