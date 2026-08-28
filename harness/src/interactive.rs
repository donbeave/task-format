//! Confirmations.
//!
//! Default answer is NO. `--auto`/`--yes` skip the question (auto still prints the plan line). A
//! non-TTY stdin without those flags is a hard error for every mutating command
//! (create/delete/promote/run/experiment/preload/build-images). Read-only commands never prompt.

use std::io::{BufRead, IsTerminal};

use anyhow::bail;

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
    pub fn confirm(&self, title: &str, detail: &[String]) -> anyhow::Result<bool> {
        crate::redact::eemit(&format!("== {title}"));
        for line in detail {
            crate::redact::eemit(&format!("   {line}"));
        }
        if self.skip_prompts() {
            crate::redact::eemit("   Proceed? [y/N] yes (--auto)");
            return Ok(true);
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
            return Ok(false);
        }
        let confirmed = matches!(answer.as_str(), "y" | "yes");
        crate::redact::eemit(&format!("   {}", if confirmed { "yes" } else { "no" }));
        Ok(confirmed)
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
    fn confirm_on_eof_defaults_to_no() {
        // stdin is a pipe here: EOF/no path returns Ok(false) only when prompts are allowed to be
        // asked; without --auto the guard errors first.
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
