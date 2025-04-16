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

    #[derive(Debug)]
    enum TrackingStep {
        Starved(Grams),
        Ate { eaten: Grams, key: food::Key, out_count: Grams },
        Bought(Grams, food::Key),
    }

    fn simulate(
        rng: &mut Xs,
        study: &mut Shelf,
        tracking_steps: &mut Vec<TrackingStep>,
        food_types: &FoodTypes,
        event: Event
    ) {
        macro_rules! buy {
            ($food: expr) => {
                // TODO handle money when that is implemented

                let food = $food;

                tracking_steps.push(TrackingStep::Bought(food.grams, food.key.clone()));
                study.shelf.push(food);
            }
        }

        match event {
            Event::Ate(key, grams, .. ) => {
                fn eat_at(
                    study: &mut Shelf,
                    tracking_steps: &mut Vec<TrackingStep>,
                    index: usize,
                    grams: Grams
                ) {
                    if index >= study.shelf.len() {
                        study.perf.starved_count += 1;
                        tracking_steps.push(TrackingStep::Starved(grams));
                        return
                    }
                    let food = &mut study.shelf[index];
                    if let Some(subtracted) = food.grams.checked_sub(grams) {
                        // Base case
                        food.grams = subtracted;
                        tracking_steps.push(TrackingStep::Ate {
                            eaten: grams,
                            key: food.key.clone(),
                            out_count: 0,
                        });
                    } else {
                        let remaining_grams = grams - food.grams;
                        tracking_steps.push(TrackingStep::Ate {
                            eaten: food.grams,
                            key: food.key.clone(),
                            out_count: remaining_grams,
                        });
                        study.perf.out_count += remaining_grams;
                        food.grams = 0;

                        study.shelf.swap_remove(index);

                        // TODO? Allow configuring this? Make random an option?
                        let arbitrary_index = 0;

                        eat_at(study, tracking_steps, arbitrary_index, remaining_grams);
                    }
                }

                if let Some(index) = study.shelf.iter().position(|f| f.key == key) {
                    eat_at(study, tracking_steps, index, grams);
                } else {
                    study.perf.out_count += grams;

                    // TODO? Allow configuring this? Make random an option?
                    let arbitrary_index = 0;

                    eat_at(study, tracking_steps, arbitrary_index, grams);
                }
            },
            Event::Bought(food) => {
                buy!(food);
            }
            Event::BuyIfHalfEmpty(BuyIfHalfEmptyParams { max_count, offset }) => {
                // buy one of each kind of food if there isn't a more than half empty version of it there.
                let mut count = 0;

                for type_ in food_types.iter() {
                    let mut have_half_full = false;

                    // TODO? avoid O(n^2) here?
                    for i in 0..study.shelf.len() {
                        let food = &study.shelf[(i + offset) % study.shelf.len()];
                        if type_.key == food.key && !food.is_half_empty() {
                            have_half_full = true;
                            break
                        }
                    }

                    if !have_half_full && count < max_count {
                        buy!(Food::from_rng_of_type(&type_, rng));
                        count += 1;
                    }
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

    #[derive(Clone, Debug)]
    enum EventEntry {
        InitialDayMarker,
        DayMarker,
        Event(Event),
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

        type Events = Vec<EventEntry>;

        let mut events: Events = Vec::with_capacity(day_count);

        struct EventSourceBundle<'rng, 'food_types, F>
        where
            F: FnMut(Event)
        {
            push_event: F,
            rng: &'rng mut Xs,
            food_types: &'food_types FoodTypes,
        }

        fn buy_if_half_empty<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                ..
            }: EventSourceBundle<F>,
            params: &BuyIfHalfEmptyParams,
        ) {
            push_event(Event::BuyIfHalfEmpty(params.clone()));
        }

        fn buy_random_variety<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                rng,
                food_types,
                ..
            }: EventSourceBundle<F>,
            BuyRandomVarietyParams { count, offset }: &BuyRandomVarietyParams,
        ) {
            for i in 0..*count {
                let index = (i as usize).wrapping_add(*offset) % food_types.len();
                push_event(Event::Bought(Food::from_rng_of_type(&food_types[index], rng)));
            }
        }

        fn fixed_hunger_amount<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                rng,
                food_types,
                ..
            }: EventSourceBundle<F>,
            FixedHungerAmountParams { grams_per_day }: &FixedHungerAmountParams,
        ) {
            let mut grams_remaining = *grams_per_day;
            while grams_remaining > 0 {
                let index = xs::range(rng, 0..food_types.len() as u32) as usize;

                let type_ = &food_types[index];

                // TODO Define a serving on each food type, and eat say 1.5 to 2.5 of them each time
                let amount = xs::range(rng, 1..(grams_remaining as u32 + 1)) as Grams;

                push_event(Event::Ate(
                    type_.key.clone(),
                    amount,
                ));

                grams_remaining = grams_remaining.saturating_sub(amount as _);
            }
        }

        fn shop_some_days<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                rng,
                food_types,
                ..
            }: EventSourceBundle<F>,
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
                        push_event(Event::Bought(Food::from_rng(food_types, rng)));
                    }
                },
                _ => {
                    // Skip shopping
                }
            }
        }

        fn random_event<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                rng,
                food_types,
                ..
            }: EventSourceBundle<F>,
            RandomEventParams {
                roll_one_past_max,
            }: &RandomEventParams,
        ) {
            // Have random things happen sometimes as an attempt to capture things not explicitly modeled
            match xs::range(rng, 0..roll_one_past_max.u32()) {
                0 => {
                    push_event(Event::from_rng(food_types, rng));
                },
                _ => {}
            }
        }

        macro_rules! b {
            () => {
                EventSourceBundle {
                    push_event: |e| events.push(EventEntry::Event(e)),
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

        events.push(EventEntry::InitialDayMarker);

        for _ in 0..day_count {
            get_events!(repeated_event_source_specs);

            events.push(EventEntry::DayMarker);
        }
        assert!(events.len() > food_types.len());

        let mut all_stats = Vec::with_capacity(events.len() + 1);

        let mut tracking_steps = Vec::with_capacity(16);

        let mut daily_ate_total = 0;
        let mut daily_bought_total = 0;

        for event_entry in events {
            match event_entry {
                EventEntry::InitialDayMarker => {
                    if spec.show_step_by_step {
                        writeln!(w, "======= Start of the First Day ==========")?;
                        daily_ate_total = 0;
                        daily_bought_total = 0;
                    }
                }
                EventEntry::DayMarker => {
                    if spec.show_step_by_step {
                        writeln!(w, "============= End of Day ================")?;
                        writeln!(w, "Ate: {daily_ate_total}")?;
                        writeln!(w, "Bought: {daily_bought_total}")?;
                        writeln!(w, "=========================================")?;
                        daily_ate_total = 0;
                        daily_bought_total = 0;
                    }
                },
                EventEntry::Event(event) => {
                    all_stats.push(stats(&study));

                    tracking_steps.clear();

                    simulate(
                        &mut rng,
                        &mut study,
                        &mut tracking_steps,
                        &food_types,
                        event
                    );
        
                    if spec.show_step_by_step {
                        use TrackingStep::*;
                        for step in &tracking_steps {
                            match step {
                                Starved(grams) => {
                                    writeln!(w, "Starved by {grams}g")?;
                                }
                                Ate { eaten, key, out_count } => {
                                    writeln!(w, "Ate {eaten}g of {key}")?;

                                    if *out_count > 0 {
                                        writeln!(w, "    Ran out by {out_count}g")?;
                                    }
                                    
                                    daily_ate_total += eaten;
                                },
                                Bought(grams, key) => {
                                    writeln!(w, "Bought {grams}g of {key}")?;
                                    daily_bought_total += grams;
                                },
                            }
                        }
                    }
                },
            }
        }

        if spec.show_step_by_step {
            writeln!(w, "")?;
        }
        drop(tracking_steps);

        if spec.show_grams {
            writeln!(w, "grams: [")?;
            for stats in &all_stats {
                writeln!(w, "    {}", stats.total_grams)?;
            }
            writeln!(w, "]")?;

            writeln!(w, "")?;
        }

        if spec.show_items {
            writeln!(w, "items: [")?;
            for stats in &all_stats {
                writeln!(w, "    {}", stats.total_items)?;
            }
            writeln!(w, "]")?;

            writeln!(w, "")?;
        }

        if !spec.hide_summary {
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
        }

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