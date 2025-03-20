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

mod basic {
    use std::io::Write;
    use crate::types::Spec;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Jam
    }

    // 64k grams ouught to be enough for anybody!
    type Grams = u16;

    #[derive(Debug)]
    struct Food {
        kind: Kind,
        grams: Grams,
        // TODO expiry date. Or maybe in a different model
    }

    #[derive(Default)]
    struct Shelf {
        shelf: Vec<Food>,
    }

    fn simulate(study: &mut Shelf, event: Event) {
        match event {
            Event::Ate(kind, grams) => {
                // TODO Consider handling this error case in a better way. Perhaps not simply returning early.
                let index = study.shelf.iter().position(|f| f.kind == kind).unwrap();

                // TODO Consider handling this error case in a better way. Perhaps not simply returning early.
                // For example, maybe eating a random different food, and recording that happened, so we can
                // use that as a marker of performance.
                study.shelf[index].grams = study.shelf[index].grams.checked_sub(grams).unwrap();
            },
            Event::Bought(kind, grams) => {
                study.shelf.push(Food{
                    kind,
                    grams,
                });
            }
        }
    }

    #[derive(Clone)]
    enum Event {
        Ate(Kind, Grams),
        Bought(Kind, Grams),
    }

    pub fn run(_spec: &Spec, mut w: impl Write) -> Result<(), std::io::Error> {
        let mut study: Shelf = Shelf::default();

        // TODO Generate random events
        // TODO Make food types definable in the config

        let mut events = vec![Event::Ate(Kind::Jam, 30); 10];
        events.insert(0, Event::Bought(Kind::Jam, 300));

        for event in events {
            writeln!(w, "{:?}", study.shelf)?;

            simulate(&mut study, event);
        }

        writeln!(w, "{:?}", study.shelf)?;

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
        Basic => {
            basic::run(&spec, &output)?;
        }
    }

    Ok(())
}