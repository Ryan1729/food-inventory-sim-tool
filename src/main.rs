mod types;
use types::{Mode, Res, Spec};
mod config;

mod minimal {
    use std::io::Write;

    struct GummyBear;

    type Minimal = Option<GummyBear>;

    pub fn run(mut w: impl Write) -> Result<(), std::io::Error> {
        let mut study: Minimal = None;

        writeln!(w, "{}", study.is_some())?;

        study = Some(GummyBear);

        writeln!(w, "{}", study.is_some())?;

        study = None;

        writeln!(w, "{}", study.is_some())?;

        Ok(())
    }
}

fn main() -> Res<()> {
    use Mode::*;
    let spec: Spec = config::get_spec()?;

    let output = std::io::stdout();

    match spec.mode {
        Minimal => {
            minimal::run(&output)?;
        }
    }

    Ok(())
}