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
    use crate::types::{*, FoodTypes, food::{self, Grams}, Spec};

    #[derive(Clone, Debug)]
    struct Food {
        key: food::Key,
        option: food::Option,
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

            Self::of_type(&type_, option.clone())
        }

        fn of_type(type_: &food::Type, option: food::Option) -> Self {
            Self {
                key: type_.key.clone(),
                grams: option.grams, // Full of the current grams
                option: option,
            }
        }

        pub fn is_half_empty(&self) -> bool {
            self.grams <= (self.option.grams / 2)
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

    fn simulate(
        rng: &mut Xs,
        study: &mut Shelf,
        food_types: &FoodTypes,
        event: Event
    ) {
        macro_rules! buy {
            ($food: expr) => {
                // TODO handle money when that is implemented

                study.shelf.push($food);
            }
        }

        match event {
            Event::Ate(key, grams, .. ) => {
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
                buy!(food);
            }
            Event::BuyIfHalfEmpty(BuyIfHalfEmptyParams { max_count, offset }) => {
                let len = study.shelf.len();
                if len == 0 {
                    // TODO? Fallback to another strat?
                    return
                }

                let mut index = offset % len;
                let count = core::cmp::min(
                    ShoppingCount::MAX as usize,
                    core::cmp::min(max_count as usize, len)
                ) as ShoppingCount;

                for _ in 0..count {
                    let food = &study.shelf[index];
                    if food.is_half_empty() {
                        // TODO? avoid O(n^2) here?
                        for type_ in food_types.iter() {
                            if type_.key == food.key {
                                buy!(Food::from_rng_of_type(&type_, rng));

                                break
                            }
                        }
                    }
                    index += 1;
                    index %= len;
                }
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

    #[derive(Clone, Debug)]
    enum Event {
        Ate(food::Key, Grams),
        Bought(Food),
        BuyIfHalfEmpty(BuyIfHalfEmptyParams)
    }

    impl Event {
        fn from_rng(food_types: &FoodTypes, rng: &mut Xs) -> Self {
            match xs::range(rng, 0..2 as u32) {
                1 => Self::Bought(Food::from_rng(food_types, rng)),
                _ => {
                    let food = Food::from_rng(food_types, rng);
                    Self::Ate(food.key, food.grams)
                },
            }
        }
    }

    pub fn run(spec: &Spec, mut w: impl Write) -> Result<(), std::io::Error> {
        let crate::types::BasicExtras {
            food_types,
            initial_event_source_specs,
            repeated_event_source_specs,
        } = match &spec.mode {
            crate::Mode::Basic(extras) => {
                extras
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

        struct EventSourceBundle<'events, 'rng, 'food_types> {
            events: &'events mut Events,
            rng: &'rng mut Xs,
            food_types: &'food_types FoodTypes,
        }

        fn buy_if_half_empty(
            EventSourceBundle {
                events,
                ..
            }: EventSourceBundle,
            params: &BuyIfHalfEmptyParams,
        ) {
            events.push(Event::BuyIfHalfEmpty(params.clone()));
        }

        fn buy_random_variety(
            EventSourceBundle {
                events,
                rng,
                food_types,
                ..
            }: EventSourceBundle,
            BuyRandomVarietyParams { count, offset }: &BuyRandomVarietyParams,
        ) {
            for i in 0..*count {
                let index = (i as usize).wrapping_add(*offset) % food_types.len();
                events.push(Event::Bought(Food::from_rng_of_type(&food_types[index], rng)));
            }
        }

        fn fixed_hunger_amount(
            EventSourceBundle {
                events,
                rng,
                food_types,
                ..
            }: EventSourceBundle,
            FixedHungerAmountParams { grams_per_day }: &FixedHungerAmountParams,
        ) {
            let mut grams_remaining = *grams_per_day;
            while grams_remaining > 0 {
                let index = xs::range(rng, 0..food_types.len() as u32) as usize;

                let type_ = &food_types[index];

                // TODO Define a serving on each food type, and eat say 1.5 to 2.5 of them each time
                let amount = xs::range(rng, 1..(grams_remaining as u32 + 1)) as Grams;

                events.push(Event::Ate(
                    type_.key.clone(),
                    amount,
                ));

                grams_remaining = grams_remaining.saturating_sub(amount as _);
            }
        }

        // TODO An actual reasonable purchase strategy
        //    Something based on the threshold of how much of each food we have left
        fn shop_some_days(
            EventSourceBundle {
                events,
                rng,
                food_types,
                ..
            }: EventSourceBundle,
            ShopSomeDaysParams {
                buy_count,
                roll_one_past_max,
            }: &ShopSomeDaysParams,
        ) {
            match xs::range(rng, 0..roll_one_past_max.u32()) {
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
                food_types,
                ..
            }: EventSourceBundle,
            RandomEventParams {
                roll_one_past_max,
            }: &RandomEventParams,
        ) {
            // Have random things happen sometimes as an attempt to capture things not explicitly modeled
            match xs::range(rng, 0..roll_one_past_max.u32()) {
                0 => {
                    events.push(Event::from_rng(food_types, rng));
                },
                _ => {}
            }
        }

        macro_rules! b {
            () => {
                EventSourceBundle {
                    events: &mut events,
                    rng: &mut rng,
                    food_types: &food_types,
                }
            }
        }

        macro_rules! get_events {
            ($es_specs: expr) => {
                for es_spec in $es_specs.iter() {
                    match es_spec {
                        EventSourceSpec::BuyIfHalfEmpty(p) => buy_if_half_empty(b!(), &p),
                        EventSourceSpec::BuyRandomVariety(p) => buy_random_variety(b!(), &p),
                        EventSourceSpec::FixedHungerAmount(p) => fixed_hunger_amount(b!(), &p),
                        EventSourceSpec::ShopSomeDays(p) => shop_some_days(b!(), &p),
                        EventSourceSpec::RandomEvent(p) => random_event(b!(), &p),
                    }
                }
            }
        }

        get_events!(initial_event_source_specs);

        for _ in 0..day_count {
            get_events!(repeated_event_source_specs);
        }
        assert!(events.len() > food_types.len());

        let mut all_stats = Vec::with_capacity(events.len() + 1);

        for event in events {
            all_stats.push(stats(&study));

            simulate(&mut rng, &mut study, &food_types, event);
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
        let mut out_count: Grams = 0;
        let mut starved_count: u16 = 0;

        for stats in &all_stats {
            performance = core::cmp::max(performance, stats.snapshot.performance());
            out_count = core::cmp::max(out_count, stats.snapshot.out_count);
            starved_count = core::cmp::max(starved_count, stats.snapshot.starved_count);
        }

        writeln!(w, "out_count (closer to 0 is better): {out_count}")?;
        writeln!(w, "starved_count (closer to 0 is better): {starved_count}\n")?;
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