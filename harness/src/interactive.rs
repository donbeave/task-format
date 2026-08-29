//! Confirmations.
//!
//! Default answer is NO. `--auto`/`--yes` skip the question (auto still prints the plan line). A
//! non-TTY stdin without those flags is a hard error for every mutating command
//! (create/delete/promote/run/experiment/preload/build-images). Read-only commands never prompt.

use std::io::{BufRead, IsTerminal};
use std::ops::Not;

use anyhow::bail;

/// A confirmation answer. The `must_use` is the whole point of the type: an answer produced and
/// then dropped in statement position is `unused_must_use`, and the harness builds under
/// `-D warnings`. A `bool` in the same position is silent.
#[must_use = "an unchecked confirmation answer is a consent bug"]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Confirmed,
    Declined,
}

impl Decision {
    /// Turn the answer into a decision the caller's `?` cannot step over: `Ok` when confirmed, and
    /// on a decline an error reading `declined: not <action>`.
    pub fn or_decline(self, action: &str) -> anyhow::Result<()> {
        match self {
            Decision::Confirmed => Ok(()),
            Decision::Declined => bail!("declined: not {action}"),
        }
    }
}

/// `!answer` is true when the answer was no. This exists so a caller that has its own refusal
/// behaviour (its own message, its own exit code) keeps reading `if !...confirm(...)?`.
impl Not for Decision {
    type Output = bool;
    fn not(self) -> bool {
        self == Decision::Declined
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Interaction {
    pub auto: bool,
    pub yes: bool,
}

impl Interaction {
    pub fn new(auto: bool, yes: bool) -> Self {
        Self { auto, yes }
    }

    pub fn skip_prompts(&self) -> bool {
        self.auto || self.yes
    }

    /// mutating commands: refuse to proceed non-interactively without an explicit flag
    pub fn require_consent_source(&self, command: &str) -> anyhow::Result<()> {
        if self.skip_prompts() {
            return Ok(());
        }
        if !std::io::stdin().is_terminal() {
            bail!(
                "{command} would change state and stdin is not a terminal — pass --auto (or --yes) to proceed non-interactively"
            );
        }
        Ok(())
    }

    /// Ask `Proceed? [y/N]` after printing the plan. Default NO.
    ///
    /// ```
    /// use taskfmt::interactive::{Decision, Interaction};
    /// let interaction = Interaction::new(true, false);
    /// assert_eq!(interaction.confirm("doc", &[]).unwrap(), Decision::Confirmed);
    /// ```
    ///
    /// The answer cannot be thrown away: this does not compile.
    ///
    /// ```compile_fail
    /// #![deny(unused_must_use)]
    /// let interaction = taskfmt::interactive::Interaction::new(true, false);
    /// interaction.confirm("doc", &[]).unwrap();
    /// ```
    pub fn confirm(&self, title: &str, detail: &[String]) -> anyhow::Result<Decision> {
        crate::redact::eemit(&format!("== {title}"));
        for line in detail {
            crate::redact::eemit(&format!("   {line}"));
        }
        if self.skip_prompts() {
            crate::redact::eemit("   Proceed? [y/N] yes (--auto)");
            return Ok(Decision::Confirmed);
        }
        if !std::io::stdin().is_terminal() {
            bail!(
                "refusing to {title} without an answer: stdin is not a terminal — pass --auto or --yes"
            );
        }
        crate::redact::eemit("   Proceed? [y/N]");
        let mut line = String::new();
        let read = std::io::stdin().lock().read_line(&mut line)?;
        let answer = line.trim().to_ascii_lowercase();
        if read == 0 {
            crate::redact::eemit("   no (eof)");
            return Ok(Decision::Declined);
        }
        let confirmed = matches!(answer.as_str(), "y" | "yes");
        crate::redact::eemit(&format!("   {}", if confirmed { "yes" } else { "no" }));
        Ok(if confirmed {
            Decision::Confirmed
        } else {
            Decision::Declined
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_and_yes_skip_prompts() {
        assert!(Interaction::new(true, false).skip_prompts());
        assert!(Interaction::new(false, true).skip_prompts());
        assert!(!Interaction::new(false, false).skip_prompts());
    }

    #[test]
    fn piped_stdin_is_not_a_terminal_and_blocks_mutations() {
        // cargo test runs without a tty, so the guard must fire
        let err = Interaction::new(false, false)
            .require_consent_source("preload")
            .unwrap_err();
        assert!(err.to_string().contains("--auto"), "{err}");
        assert!(
            Interaction::new(true, false)
                .require_consent_source("preload")
                .is_ok()
        );
        assert!(
            Interaction::new(false, true)
                .require_consent_source("preload")
                .is_ok()
        );
    }

    #[test]
    fn a_declined_answer_is_an_error_that_cannot_be_stepped_over() {
        Decision::Confirmed.or_decline("deleting").unwrap();
        println!("DECISION confirmed=ok");
        let err = Decision::Declined.or_decline("deleting").unwrap_err();
        println!("DECISION declined={err}");
        assert_eq!(err.to_string(), "declined: not deleting");
        assert_eq!(
            Decision::Declined
                .or_decline("creating")
                .unwrap_err()
                .to_string(),
            "declined: not creating"
        );
        assert_eq!(
            Decision::Declined
                .or_decline("promoting")
                .unwrap_err()
                .to_string(),
            "declined: not promoting"
        );
    }

    #[test]
    fn the_consent_flags_still_answer_yes_without_reading_stdin() {
        let auto = Interaction::new(true, false)
            .confirm("t", &["plan".to_string()])
            .unwrap();
        let yes = Interaction::new(false, true).confirm("t", &[]).unwrap();
        println!("SEMANTICS auto={auto:?} yes={yes:?}");
        assert_eq!(auto, Decision::Confirmed);
        assert_eq!(yes, Decision::Confirmed);
        if !std::io::stdin().is_terminal() {
            let err = Interaction::new(false, false)
                .confirm("delete thing", &[])
                .unwrap_err();
            assert!(err.to_string().contains("stdin is not a terminal"), "{err}");
        }
    }

    #[test]
    fn confirm_on_eof_defaults_to_no() {
        // stdin is a pipe here: the EOF/no path returns Ok(Decision::Declined) only when prompts
        // are allowed to be asked; without --auto the guard errors first.
        let interaction = Interaction::new(true, false);
        assert!(interaction.confirm("title", &["plan".to_string()]).unwrap());
    }

    #[test]
    fn confirm_without_flag_on_a_pipe_errors() {
        let err = Interaction::new(false, false)
            .confirm("delete thing", &[])
            .unwrap_err();
        assert!(err.to_string().contains("stdin is not a terminal"), "{err}");
    }
}
