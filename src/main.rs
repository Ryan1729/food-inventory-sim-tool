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

        struct FixedHungerParams {
            grams_per_day: Grams,
        }

        /// One past max value of a die to roll from 0 to. So a value of 6 indicates a roll between 6 values from
        /// 0 to 5 inclusive. Often used where somthing happens on a roll of 0, and nothing otherwise.
        // TODO a more intuitive representation of the roll being made.
        type RollOnePastMax = u8;

        struct ShopSomeDaysParams {
            buy_count: u8,
            roll_one_past_max: RollOnePastMax,
        }

        struct RandomEventParams {
            roll_one_past_max: RollOnePastMax,
        }

        enum EventSourceSpec {
            FixedHungerAmount(FixedHungerParams),
            ShopSomeDays(ShopSomeDaysParams),
            RandomEvent(RandomEventParams),
        }

        // TODO Make hunger models and purchase strategies configurable
        struct EventSourceBundle<'events, 'rng, 'food_types> {
            events: &'events mut Events,
            rng: &'rng mut Xs,
            food_types: &'food_types FoodTypes
        }

        type EventSource<Params> = fn (
            EventSourceBundle<'_, '_, '_>,
            Params
        );

        fn fixed_hunger_amount(
            EventSourceBundle {
                events,
                rng,
                food_types
            }: EventSourceBundle,
            FixedHungerParams { grams_per_day }: &FixedHungerParams,
        ) {
            let mut grams_remaining = *grams_per_day;
            while grams_remaining > 0 {
                let index = xs::range(rng, 0..food_types.len() as u32) as usize;

                let type_ = &food_types[index];

                // TODO Define a serving on each food type, and eat say 1.5 to 2.5 of them each time
                let amount = xs::range(rng, 1..(grams_remaining as u32 + 1)) as Grams;

                events.push(Event::Ate(Food{
                    key: type_.key.clone(),
                    grams: amount,
                }));

                grams_remaining = grams_remaining.saturating_sub(amount as _);
            }
        }

        fn shop_some_days(
            EventSourceBundle {
                events,
                rng,
                food_types
            }: EventSourceBundle,
            ShopSomeDaysParams {
                buy_count,
                roll_one_past_max,
            }: &ShopSomeDaysParams,
        ) {
            match xs::range(rng, 0..(*roll_one_past_max as u32)) {
                0 => {
                    // Go shopping
                    // TODO Count grams and buy a set amount of grams instead of an item count?
                    for _ in 0..*buy_count {
                        events.push(Event::Bought(Food::from_rng(food_types, rng)));
                    }
                },
                _ => {
                    // Skip shopping
                }
            }
        }

        fn random_event(
            EventSourceBundle {
                events,
                rng,
                food_types
            }: EventSourceBundle,
            RandomEventParams {
                roll_one_past_max,
            }: &RandomEventParams,
        ) {
            // Have random things happen sometimes as an attempt to capture things not explicitly modeled
            match xs::range(rng, 0..(*roll_one_past_max as u32)) {
                0 => {
                    events.push(Event::from_rng(food_types, rng));
                },
                _ => {}
            }
        }

        let event_source_specs = vec![
            EventSourceSpec::FixedHungerAmount(FixedHungerParams {
                grams_per_day: 2000,
            }),
            EventSourceSpec::ShopSomeDays(ShopSomeDaysParams {
                buy_count: 3,
                roll_one_past_max: 4,
            }),
            EventSourceSpec::RandomEvent(RandomEventParams {
                roll_one_past_max: 16,
            }),
        ];

        macro_rules! b {
            () => { EventSourceBundle { events: &mut events, rng: &mut rng, food_types: &food_types } }
        }

        for _ in 0..day_count {
            for es_spec in &event_source_specs {
                match es_spec {
                    EventSourceSpec::FixedHungerAmount(p) => fixed_hunger_amount(b!(), p),
                    EventSourceSpec::ShopSomeDays(p) => shop_some_days(b!(), p),
                    EventSourceSpec::RandomEvent(p) => random_event(b!(), p),
                }
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