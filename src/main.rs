mod xs;
mod minimize;
mod types;
use types::{Mode, Res, Spec, PrintCallsSpec};
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

        #[allow(unused)]
        pub fn is_at_least_this_full(&self, fullness_threshold: FullnessThreshold) -> bool {
            self.grams as f32 >= (self.option.grams as f32 * fullness_threshold)
        }

        pub fn current_fullness(&self) -> f32 {
            self.grams as f32 / self.option.grams as f32
        }
    }

    pub type Performance = u32;

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
        Ate { eaten: Grams, key: food::Key, out_count: Grams, servings_count: f32 },
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

        struct ShelfIndex(usize);

        fn calc_servings_count(
            food_types: &FoodTypes,
            key: &food::Key,
            grams: Grams,
        ) -> f32 {
            food_types.iter().find(|type_| &type_.key == key)
                .map(|type_| {
                    grams as f32 / type_.serving as f32
                })
                .unwrap_or(-f32::INFINITY)
        }

        match event {
            Event::Ate(key, grams, .. ) => {
                fn eat_at(
                    study: &mut Shelf,
                    tracking_steps: &mut Vec<TrackingStep>,
                    index: ShelfIndex,
                    grams: Grams,
                    food_types: &FoodTypes,
                ) {
                    if index.0 >= study.shelf.len() {
                        study.perf.starved_count += 1;
                        tracking_steps.push(TrackingStep::Starved(grams));
                        return
                    }
                    let food = &mut study.shelf[index.0];
                    if let Some(subtracted) = food.grams.checked_sub(grams) {
                        // Base case
                        food.grams = subtracted;
                        tracking_steps.push(TrackingStep::Ate {
                            eaten: grams,
                            key: food.key.clone(),
                            out_count: 0,
                            servings_count: calc_servings_count(food_types, &food.key, grams),
                        });
                    } else {
                        let remaining_grams = grams - food.grams;
                        tracking_steps.push(TrackingStep::Ate {
                            eaten: food.grams,
                            key: food.key.clone(),
                            out_count: remaining_grams,
                            servings_count: calc_servings_count(food_types, &food.key, grams),
                        });
                        study.perf.out_count += remaining_grams;
                        food.grams = 0;

                        study.shelf.swap_remove(index.0);

                        // TODO? Allow configuring this? Make random an option?
                        let arbitrary_index = ShelfIndex(0);

                        eat_at(study, tracking_steps, arbitrary_index, remaining_grams, food_types);
                    }
                }

                if let Some(index) = study.shelf.iter().position(|f| f.key == key) {
                    eat_at(study, tracking_steps, ShelfIndex(index), grams, food_types);
                } else {
                    study.perf.out_count += grams;

                    // TODO? Allow configuring this? Make random an option?
                    let arbitrary_index = ShelfIndex(0);

                    eat_at(study, tracking_steps, arbitrary_index, grams, food_types);
                }
            },
            Event::Bought(food) => {
                buy!(food);
            }
            Event::BuyAllBasedOnFullness(
                BuyAllBasedOnFullnessParams {
                    max_count,
                    offset,
                    fullness_threshold
                }
            ) => {
                // buy one of each kind of food if there isn't a more than fullness_threshold full version of it there.
                let mut count = 0;

                for type_ in food_types.iter() {
                    let mut total_fullness = 0.;

                    // TODO? avoid O(n^2) here? Like maybe calcualting all the totals AOT say?
                    for i in 0..study.shelf.len() {
                        let food = &study.shelf[(i + offset) % study.shelf.len()];
                        if type_.key == food.key {
                            total_fullness += food.current_fullness();
                            break
                        }
                    }

                    if total_fullness < fullness_threshold && count < max_count {
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
        BuyAllBasedOnFullness(BuyAllBasedOnFullnessParams)
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

    pub struct RunOutput {
        pub performance: Performance,
    }

    pub fn run(spec: &Spec, mut w: impl Write) -> Result<RunOutput, std::io::Error> {
        let crate::types::BasicExtras {
            mode: _,
            food_types,
            initial_event_source_specs,
            repeated_event_source_specs,
        } = match &spec.mode {
            crate::Mode::Basic(extras) => {
                extras
            },
            crate::Mode::Minimal => {
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

        fn buy_if_below_threshold<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                ..
            }: EventSourceBundle<F>,
            params: &BuyAllBasedOnFullnessParams,
        ) {
            push_event(Event::BuyAllBasedOnFullness(params.clone()));
        }

        fn buy_if_half_empty<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                ..
            }: EventSourceBundle<F>,
            params: &BuyIfHalfEmptyParams,
        ) {
            push_event(Event::BuyAllBasedOnFullness(BuyAllBasedOnFullnessParams {
                max_count: params.max_count,
                offset: params.offset,
                fullness_threshold: 0.5,
            }));
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

        // TODO more realistic hunger model with meals

        fn fixed_servings_amount<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                rng,
                food_types,
                ..
            }: EventSourceBundle<F>,
            FixedServingsAmountParams { servings_per_day }: &FixedServingsAmountParams,
        ) {
            let mut g_state = xs::GaussianState::default();

            let mut servings_remaining = (*servings_per_day) as f32;
            while servings_remaining > 0. {
                let index = xs::range(rng, 0..food_types.len() as u32) as usize;

                let type_ = &food_types[index];

                // At least one serving, and as much as four.
                let servings_count = 1. + xs::gaussian_zero_to_one(rng, &mut g_state) * 3.;

                let serving_grams = type_.serving;

                let amount =
                    (servings_count * serving_grams as f32) as food::Grams;

                push_event(Event::Ate(
                    type_.key.clone(),
                    amount,
                ));

                servings_remaining -= servings_count;
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
                        EventSourceSpec::BuyIfBelowThreshold(p) => buy_if_below_threshold(b!(), &p),
                        EventSourceSpec::BuyIfHalfEmpty(p) => buy_if_half_empty(b!(), &p),
                        EventSourceSpec::BuyRandomVariety(p) => buy_random_variety(b!(), &p),
                        EventSourceSpec::FixedHungerAmount(p) => fixed_hunger_amount(b!(), &p),
                        EventSourceSpec::FixedServingsAmount(p) => fixed_servings_amount(b!(), &p),
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
                        writeln!(w, "Stock:")?;

                        for item in &study.shelf {
                            writeln!(w, "    {}: {}g", item.key, item.grams)?;
                        }

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
                                Ate { eaten, key, out_count, servings_count } => {
                                    // TODO? label when it's a substitute thing, so lots of servings are expected?
                                    writeln!(w, "Ate {eaten}g of {key} ({servings_count} servings)")?;

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

        let mut performance: Performance = 0;
        let mut out_count: Grams = 0;
        let mut starved_count: u16 = 0;

        for stats in &all_stats {
            performance = core::cmp::max(performance, stats.snapshot.performance());
            out_count = core::cmp::max(out_count, stats.snapshot.out_count);
            starved_count = core::cmp::max(starved_count, stats.snapshot.starved_count);
        }

        if !spec.hide_summary {
            writeln!(w, "out_count (closer to 0 is better): {out_count}")?;
            writeln!(w, "starved_count (closer to 0 is better): {starved_count}\n")?;
            writeln!(w, "performance (closer to 0 is better): {performance},")?;
        }

        Ok(RunOutput {
            performance,
        })
    }
}

struct DummyWrite {}

impl std::io::Write for &DummyWrite {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> { Ok(data.len()) }

    fn flush(&mut self) -> Result<(), std::io::Error> { Ok(()) }
}

impl std::io::Write for DummyWrite {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> { Ok(data.len()) }

    fn flush(&mut self) -> Result<(), std::io::Error> { Ok(()) }
}

fn main() -> Res<()> {
    use Mode::*;
    use crate::types::{BasicExtras, EventSourceSpec, BasicMode, Target};

    use std::io::Write;

    let spec: Spec = config::get_spec()?;

    let output = std::io::stdout();

    match spec.mode {
        Minimal => {
            minimal::run(&spec, &output)?;
        }
        Basic(ref extras) => {
            match extras.mode {
                BasicMode::Run => {
                    basic::run(&spec, &output)?;
                },
                BasicMode::PrintCalls(PrintCallsSpec{ target }) => {
                    let dummy_output = DummyWrite {};

                    let func = match target {
                        // TODO add more variants when we have more desired targets
                        Target::BuyIfBelowThresholdFullnessThreshold =>
                            |[x]: [f32; 1]| {
                                let mut repeated_event_source_specs = extras.repeated_event_source_specs.clone();

                                for ess in &mut repeated_event_source_specs {
                                    match ess {
                                        EventSourceSpec::BuyIfBelowThreshold(params) => {
                                            params.fullness_threshold = x;
                                        }
                                        _ => {}
                                    }
                                }

                                basic::run(
                                    &Spec {
                                        mode: Basic(BasicExtras {
                                            mode: BasicMode::Run,
                                            repeated_event_source_specs,
                                            ..extras.clone()
                                        }),
                                        ..spec
                                    },
                                    &dummy_output
                                ).map(|o| o.performance)
                                .unwrap_or(basic::Performance::MAX) as _
                            },
                    };

                    let mut x = 0.0;

                    let step = 1./64.;

                    writeln!(&output, "[")?;
                    while x <= 1. {
                        let y: f32 = func([x]);

                        writeln!(&output, "    ({x}, {y}),")?;

                        x += step;
                    }
                    writeln!(&output, "]")?;
                },
                BasicMode::Search(ref target) => {
                    use minimize::{Call, minimize, regular_simplex_centered_at};

                    let dummy_output = DummyWrite {};

                    let (func, center) = match target {
                        // TODO add more variants when we have more desired targets
                        Target::BuyIfBelowThresholdFullnessThreshold =>
                            (
                                |[x]: [f32; 1]| {
                                    let mut repeated_event_source_specs = extras.repeated_event_source_specs.clone();

                                    for ess in &mut repeated_event_source_specs {
                                        match ess {
                                            EventSourceSpec::BuyIfBelowThreshold(params) => {
                                                params.fullness_threshold = x;
                                            }
                                            _ => {}
                                        }
                                    }

                                    basic::run(
                                        &Spec {
                                            mode: Basic(BasicExtras {
                                                mode: BasicMode::Run,
                                                repeated_event_source_specs,
                                                ..extras.clone()
                                            }),
                                            ..spec
                                        },
                                        &dummy_output
                                    ).map(|o| o.performance)
                                    .unwrap_or(basic::Performance::MAX) as _
                                },
                                [ 0.5 ]
                            ),
                    };

                    let simplex = regular_simplex_centered_at(0.5, center);

                    writeln!(&output, "simplex: {simplex:#?},")?;

                    let Call { xs: [fullness_threshold], y: performance } = minimize(
                        func,
                        simplex,
                        100,
                    );

                    writeln!(&output, "fullness_threshold: {fullness_threshold},")?;
                    writeln!(&output, "performance (closer to 0 is better): {performance},")?;
                },
            }
        }
    }

    Ok(())
}