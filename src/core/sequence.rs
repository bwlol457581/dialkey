//! Number-input state machine (instant / "+" multi-digit).

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Window open, waiting for instant digit or '+'.
    Idle,
    /// After '+'; accumulating digits.
    MultiDigit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceEvent {
    Digit(u8),
    Start,
    Confirm,
    /// Multi-digit: launch and open working folder (configured key, default Tab).
    OpenWorkdir,
    Cancel,
    Search,
    /// Delete last buffer digit; empty multi → Idle; Idle → close typing window.
    DigitBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Launch slot with this id string.
    Launch { id: String },
    /// Open the slot working folder only (do not launch Path).
    OpenWorkdirOnly { id: String },
    /// Close the typing window (cancel / done).
    Close,
    /// Close typing window and open Search.
    OpenSearch,
    /// Buffer/UI changed; redraw candidates.
    Redraw,
    /// No-op (ignored input).
    None,
}

#[derive(Debug, Clone)]
pub struct SequenceState {
    mode: Mode,
    buffer: String,
    max_digits: usize,
    instant: HashMap<String, bool>,
}

impl SequenceState {
    pub fn new(max_digits: usize, instant: HashMap<String, bool>) -> Self {
        Self {
            mode: Mode::Idle,
            buffer: String::new(),
            max_digits: max_digits.max(1),
            instant,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn is_instant(&self, digit: u8) -> bool {
        self.instant
            .get(&digit.to_string())
            .copied()
            .unwrap_or(false)
    }

    pub fn handle(&mut self, event: SequenceEvent) -> Action {
        match event {
            SequenceEvent::Cancel => Action::Close,
            SequenceEvent::Search => Action::OpenSearch,
            SequenceEvent::Start => match self.mode {
                Mode::Idle => {
                    self.mode = Mode::MultiDigit;
                    self.buffer.clear();
                    Action::Redraw
                }
                Mode::MultiDigit => {
                    // '+' again cancels multi-digit mode
                    Action::Close
                }
            },
            SequenceEvent::Confirm => match self.mode {
                Mode::Idle => {
                    tracing::debug!("confirm ignored (idle)");
                    Action::None
                }
                Mode::MultiDigit => {
                    if self.buffer.is_empty() {
                        tracing::debug!("confirm ignored (empty buffer)");
                        Action::None
                    } else {
                        Action::Launch {
                            id: self.buffer.clone(),
                        }
                    }
                }
            },
            SequenceEvent::OpenWorkdir => match self.mode {
                Mode::Idle => Action::None,
                Mode::MultiDigit => {
                    if self.buffer.is_empty() {
                        Action::None
                    } else {
                        Action::OpenWorkdirOnly {
                            id: self.buffer.clone(),
                        }
                    }
                }
            },
            SequenceEvent::Digit(d) => match self.mode {
                Mode::Idle => {
                    if self.is_instant(d) {
                        Action::Launch { id: d.to_string() }
                    } else {
                        // Spec: ignore non-instant digits in idle (no error)
                        Action::None
                    }
                }
                Mode::MultiDigit => {
                    if self.buffer.len() >= self.max_digits {
                        Action::None
                    } else {
                        self.buffer.push(char::from(b'0' + d));
                        Action::Redraw
                    }
                }
            },
            SequenceEvent::DigitBack => match self.mode {
                Mode::Idle => Action::Close,
                Mode::MultiDigit => {
                    if self.buffer.is_empty() {
                        // Leave multi-digit → first (Idle) screen.
                        self.mode = Mode::Idle;
                        Action::Redraw
                    } else {
                        self.buffer.pop();
                        Action::Redraw
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instant_map(flags: &[(u8, bool)]) -> HashMap<String, bool> {
        flags.iter().map(|(d, v)| (d.to_string(), *v)).collect()
    }

    #[test]
    fn instant_fires_immediately() {
        let mut s = SequenceState::new(10, instant_map(&[(1, true)]));
        assert_eq!(
            s.handle(SequenceEvent::Digit(1)),
            Action::Launch { id: "1".into() }
        );
    }

    #[test]
    fn non_instant_ignored_in_idle() {
        let mut s = SequenceState::new(10, instant_map(&[(1, false)]));
        assert_eq!(s.handle(SequenceEvent::Digit(1)), Action::None);
    }

    #[test]
    fn multi_digit_then_confirm() {
        let mut s = SequenceState::new(10, HashMap::new());
        assert_eq!(s.handle(SequenceEvent::Start), Action::Redraw);
        assert_eq!(s.handle(SequenceEvent::Digit(1)), Action::Redraw);
        assert_eq!(s.handle(SequenceEvent::Digit(1)), Action::Redraw);
        assert_eq!(s.buffer(), "11");
        assert_eq!(
            s.handle(SequenceEvent::Confirm),
            Action::Launch { id: "11".into() }
        );
    }

    #[test]
    fn multi_digit_open_workdir_is_folder_only() {
        let mut s = SequenceState::new(10, instant_map(&[(1, true)]));
        s.handle(SequenceEvent::Start);
        s.handle(SequenceEvent::Digit(1));
        assert_eq!(
            s.handle(SequenceEvent::OpenWorkdir),
            Action::OpenWorkdirOnly { id: "1".into() }
        );
    }

    #[test]
    fn open_workdir_ignored_in_idle_and_empty_buffer() {
        let mut s = SequenceState::new(10, HashMap::new());
        assert_eq!(s.handle(SequenceEvent::OpenWorkdir), Action::None);
        s.handle(SequenceEvent::Start);
        assert_eq!(s.handle(SequenceEvent::OpenWorkdir), Action::None);
    }

    #[test]
    fn plus_again_cancels() {
        let mut s = SequenceState::new(10, HashMap::new());
        s.handle(SequenceEvent::Start);
        assert_eq!(s.handle(SequenceEvent::Start), Action::Close);
    }

    #[test]
    fn max_digits_stops() {
        let mut s = SequenceState::new(2, HashMap::new());
        s.handle(SequenceEvent::Start);
        s.handle(SequenceEvent::Digit(1));
        s.handle(SequenceEvent::Digit(2));
        assert_eq!(s.handle(SequenceEvent::Digit(3)), Action::None);
        assert_eq!(s.buffer(), "12");
    }

    #[test]
    fn instant_digit_usable_after_plus() {
        let mut s = SequenceState::new(10, instant_map(&[(7, true)]));
        s.handle(SequenceEvent::Start);
        s.handle(SequenceEvent::Digit(7));
        s.handle(SequenceEvent::Digit(1));
        assert_eq!(
            s.handle(SequenceEvent::Confirm),
            Action::Launch { id: "71".into() }
        );
    }

    #[test]
    fn digit_back_pops_then_returns_to_idle() {
        let mut s = SequenceState::new(10, HashMap::new());
        assert_eq!(s.handle(SequenceEvent::DigitBack), Action::Close);
        s.handle(SequenceEvent::Start);
        s.handle(SequenceEvent::Digit(1));
        s.handle(SequenceEvent::Digit(2));
        assert_eq!(s.handle(SequenceEvent::DigitBack), Action::Redraw);
        assert_eq!(s.buffer(), "1");
        assert_eq!(s.handle(SequenceEvent::DigitBack), Action::Redraw);
        assert_eq!(s.buffer(), "");
        assert_eq!(s.mode(), Mode::MultiDigit);
        assert_eq!(s.handle(SequenceEvent::DigitBack), Action::Redraw);
        assert_eq!(s.mode(), Mode::Idle);
        assert_eq!(s.handle(SequenceEvent::DigitBack), Action::Close);
    }
}
