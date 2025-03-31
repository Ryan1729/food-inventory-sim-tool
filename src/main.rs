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
    use crate::types::{FoodTypes, food::{self, Grams}, Spec};

    #[derive(Clone, Debug)]
    struct Food {
        key: food::Key,
        grams: Grams,
        // TODO expiry date. Or maybe in a different model
    }

    impl Food {
        fn from_rng(food_types: &[food::Type], rng: &mut Xs) -> Self {
            let index = xs::range(rng, 0..food_types.len() as u32) as usize;

            let type_ = &food_types[index];

            Self::from_rng_of_type(type_, rng)
        }

        fn from_rng_of_type(type_: &food::Type, rng: &mut Xs) -> Self {
            let option_index = xs::range(rng, 0..type_.options.len() as u32) as usize;
            let option = &type_.options[option_index];

            Self {
                key: type_.key.clone(),
                grams: option.grams,
            }
        }
    }

    type Performance = u32;

    /// A snapshot of the data needed to evaluate the performance metric(s) of the given set of events.
    /// That is, how well those events achieve some goal, not how long it took to simulate them.
    #[derive(Clone, Copy, Debug, Default)]
    struct PerfSnapshot {
        out_count: Grams,
        // 64k starvations ought to be enough for anybody!
        starved_count: u16
    }

    impl PerfSnapshot {
        fn performance(&self) -> Performance {
            // TODO make "buying all the time" not the optimal strat by adding costs to foods
            self.starved_count as Performance * 1000 + self.out_count as Performance
        }
    }

    #[derive(Default)]
    struct Shelf {
        shelf: Vec<Food>,
        perf: PerfSnapshot,
    }

    fn simulate(study: &mut Shelf, event: Event) {
        match event {
            Event::Ate(Food { key, grams }) => {
                fn eat_at(study: &mut Shelf, index: usize, grams: Grams) {
                    if index >= study.shelf.len() {
                        study.perf.starved_count += 1;
                        return
                    }
                    let food = &mut study.shelf[index];
                    if let Some(subtracted) = food.grams.checked_sub(grams) {
                        // Base case
                        food.grams = subtracted;
                    } else {
                        let remaining_grams = grams - food.grams;
                        study.perf.out_count += remaining_grams;
                        food.grams = 0;

                        study.shelf.swap_remove(index);

                        // TODO? Allow configuring this? Make random an option?
                        let arbitrary_index = 0;

                        eat_at(study, arbitrary_index, remaining_grams);
                    }
                }

                if let Some(index) = study.shelf.iter().position(|f| f.key == key) {
                    eat_at(study, index, grams);
                } else {
                    study.perf.out_count += grams;

                    // TODO? Allow configuring this? Make random an option?
                    let arbitrary_index = 0;

                    eat_at(study, arbitrary_index, grams);
                }
            },
            Event::Bought(food) => {
                study.shelf.push(food);
            }
        }
    }

    #[derive(Debug, Default)]
    struct Stats {
        snapshot: PerfSnapshot,
        total_items: usize,
        total_grams: Grams,
    }

    fn stats(shelf: &Shelf) -> Stats {
        let mut stats = Stats::default();

        stats.snapshot = shelf.perf.clone();

        for food in shelf.shelf.iter() {
            stats.total_grams += food.grams;
        }

        stats.total_items = shelf.shelf.len();

        stats
    }

    #[derive(Clone)]
    enum Event {
        Ate(Food),
        Bought(Food),
    }

    impl Event {
        fn from_rng(food_types: &FoodTypes, rng: &mut Xs) -> Self {
            match xs::range(rng, 0..2 as u32) {
                1 => Self::Bought(Food::from_rng(food_types, rng)),
                _ => Self::Ate(Food::from_rng(food_types, rng)),
            }
        }
    }

    pub fn run(spec: &Spec, mut w: impl Write) -> Result<(), std::io::Error> {
        let food_types = match &spec.mode {
            crate::Mode::Basic(crate::types::BasicExtras { food_types }) => {
                food_types
            },
            _ => {
                panic!("TODO get rid of this case?");
            }
        };

        let mut study: Shelf = Shelf::default();

        let mut rng = xs::from_seed(spec.seed.unwrap_or_default());

        let day_count = (xs::range(&mut rng, 1..2) * 7) as usize;

        type Events = Vec<Event>;

        let mut events: Events = Vec::with_capacity(day_count);

        // TODO An actual reasonable purchase strategy
        let initial_buy_count = food_types.len();

        for i in 0..initial_buy_count {
            events.push(Event::Bought(Food::from_rng_of_type(&food_types[i as usize], &mut rng)));
        }

        // Eat a given amount of food each day, and each evening go on 0 to 2 shopping trips.

        let shopping_buy_count = 3;

        // TODO Make hunger models and purchase strategies configurable
        // TODO allow diffrent models to have different param types.
        // TODO? Model hunger models, purchase strats, and extra variance all with one type?
        type HungerModel = fn (&mut Events, &mut Xs, &FoodTypes);

        fn fixed_amount(events: &mut Events, rng: &mut Xs, food_types: &FoodTypes) {
            let grams_per_day = 2000; // TODO? random range? Configurable

            let mut grams_remaining = grams_per_day;
            while grams_remaining > 0 {
                let index = xs::range(rng, 0..food_types.len() as u32) as usize;

                let type_ = &food_types[index];

                // TODO Define a serving on each food type, and eat say 1.5 to 2.5 of them each time
                let amount = xs::range(rng, 1..(grams_remaining + 1)) as Grams;

                events.push(Event::Ate(Food{
                    key: type_.key.clone(),
                    grams: amount,
                }));

                grams_remaining = grams_remaining.saturating_sub(amount as _);
            }
        }

        let hunger_model: HungerModel = fixed_amount;

        for _ in 0..day_count {
            hunger_model(&mut events, &mut rng, &food_types);

            match xs::range(&mut rng, 0..4) {
                0 => {
                    // Go shopping
                    // TODO Count grams and buy a set amount of grams instead of an item count?
                    for _ in 0..shopping_buy_count {
                        events.push(Event::Bought(Food::from_rng(&food_types, &mut rng)));
                    }
                },
                _ => {
                    // Skip shopping
                }
            }

            // Have random things happen sometimes as an attempt to capture things not explicitly modeled
            match xs::range(&mut rng, 0..16) {
                0 => {
                    events.push(Event::from_rng(&food_types, &mut rng));
                },
                _ => {}
            }

        }
        assert!(events.len() > food_types.len());

        let mut all_stats = Vec::with_capacity(events.len() + 1);

        for event in events {
            all_stats.push(stats(&study));

            simulate(&mut study, event);
        }

        writeln!(w, "grams: [")?;
        for stats in &all_stats {
            writeln!(w, "    {}", stats.total_grams)?;
        }
        writeln!(w, "]")?;

        writeln!(w, "")?;

        writeln!(w, "items: [")?;
        for stats in &all_stats {
            writeln!(w, "    {}", stats.total_items)?;
        }
        writeln!(w, "]")?;

        let mut performance: Performance = 0;

        for stats in &all_stats {
            performance = core::cmp::max(performance, stats.snapshot.performance());
        }

        writeln!(w, "performance (closer to 0 is better): {performance},")?;

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
        Basic { .. } => {
            basic::run(&spec, &output)?;
        }
    }

    Ok(())
}