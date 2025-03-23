mod xs;
mod types;
use types::{Mode, Res, Spec};
mod config;

mod minimal {
    use std::io::Write;
    use crate::types::Spec;
    use crate::xs;

    struct GummyBear;

    type Minimal = Option<GummyBear>;

    pub fn run(spec: &Spec, mut w: impl Write) -> Result<(), std::io::Error> {
        let mut rng = xs::from_seed(spec.seed.unwrap_or_default());

        let mut study: Minimal = if xs::range(&mut rng, 0..2) > 0 { Some(GummyBear) } else { None };

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
    use crate::xs::{self, Xs};
    use crate::types::Spec;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Jam
    }

    // 64k grams ought to be enough for anybody!
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
        out_count: Grams,
        // 64k starvations ought to be enough for anybody!
        starved_count: u16
    }

    fn simulate(study: &mut Shelf, event: Event) {
        match event {
            Event::Ate(kind, grams) => {
                fn eat_at(study: &mut Shelf, index: usize, grams: Grams) {
                    if index >= study.shelf.len() {
                        study.starved_count += 1;
                        return
                    }
                    let food = &mut study.shelf[index];
                    if let Some(subtracted) = food.grams.checked_sub(grams) {
                        // Base case
                        food.grams = subtracted;
                    } else {
                        let remaining_grams = grams - food.grams;
                        study.out_count += remaining_grams;
                        food.grams = 0;

                        study.shelf.swap_remove(index);

                        // TODO? Allow configuring this? Make random an option?
                        let arbitrary_index = 0;

                        eat_at(study, arbitrary_index, remaining_grams);
                    }
                }

                if let Some(index) = study.shelf.iter().position(|f| f.kind == kind) {
                    eat_at(study, index, grams);
                } else {
                    study.out_count += grams;

                    // TODO? Allow configuring this? Make random an option?
                    let arbitrary_index = 0;

                    eat_at(study, arbitrary_index, grams);
                }
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

    impl Event {
        fn from_rng(rng: &mut Xs) -> Self {
            match xs::range(rng, 0..2) {
                1 => Self::Bought(
                    Kind::Jam,
                    xs::range(rng, 0..(u16::MAX as u32) & u16::MAX as u32) as u16,
                ),
                _ => Self::Ate(
                    Kind::Jam,
                    xs::range(rng, 0..(u16::MAX as u32) & u16::MAX as u32) as u16,
                ),
            }
        }
    }

    pub fn run(spec: &Spec, mut w: impl Write) -> Result<(), std::io::Error> {
        let mut study: Shelf = Shelf::default();

        // TODO Make food types definable in the config

        let mut rng = xs::from_seed(spec.seed.unwrap_or_default());

        let event_count = xs::range(&mut rng, 10..16);

        let mut events = Vec::with_capacity(event_count as usize);

        events.push(Event::Bought(Kind::Jam, 300));
        for _ in 1..event_count {
            events.push(Event::from_rng(&mut rng));
        }

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
            minimal::run(&spec, &output)?;
        }
        Basic => {
            basic::run(&spec, &output)?;
        }
    }

    Ok(())
}