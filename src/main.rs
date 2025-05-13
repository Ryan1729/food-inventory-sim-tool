mod xs;
mod minimize;
mod types;
use types::{Mode, Res, Spec, SearchSpec, PrintCallsSpec};
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
            Self::of_key(type_.key.clone(), option)
        }

        fn of_key(key: food::Key, option: food::Option) -> Self {
            Self {
                key,
                grams: option.grams, // Full of the current grams
                option: option,
            }
        }

        pub fn current_fullness(
            &self,
            minimum_purchase_servings: food::Servings,
            servings_per_pack: food::NonZeroServings,
        ) -> f32 {
            // Say minimum_purchase_servings is 7, and servings_per_pack is 4.
            // We want to buy 2 servings because we need 2 4s to make at least 7.
            // 7 / 4 = 1 (integer division) so we add one to make 2.
            // this also works out with minimum_purchase_servings = 0.
            let pack_count = (minimum_purchase_servings / servings_per_pack.get()) + 1;

            let denominator = self.option.grams * pack_count;

            self.grams as f32 / denominator as f32
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
        macro_rules! calc_servings_per_pack {
            ($food: expr, $serving: expr) => ({
                let serving: food::NonZeroGrams = $serving;
                let servings_per_pack: food::GramsSizedType = $food.option.grams / serving;
                food::NonZeroServings::try_from(servings_per_pack).unwrap_or(food::NonZeroServings::MIN)
            })
        }

        macro_rules! buy {
            ($food: expr, $minimum_purchase_servings: expr) => {
                // TODO handle money when that is implemented

                let food: crate::basic::Food = $food;

                // Buy more if one is below the configured minimum number of servings.
                let mut serving = food::default_serving();
                for type_ in food_types {
                    if type_.key == food.key {
                        serving = type_.serving;
                    }
                }

                let servings_per_pack: food::NonZeroServings = calc_servings_per_pack!(food, serving);

                let minimum_purchase_servings = $minimum_purchase_servings;

                let mut servings_bought = 0;

                while {
                    tracking_steps.push(TrackingStep::Bought(food.grams, food.key.clone()));
                    // TODO? I guess we could probably do some math to eliminate the last
                    // clone? That would help in the common case of there being enough
                    // servings in one pack.
                    study.shelf.push(food.clone());

                    servings_bought += servings_per_pack.get();

                    servings_bought < minimum_purchase_servings
                } {}
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
                    grams as f32 / type_.serving.get() as f32
                })
                .unwrap_or(-f32::INFINITY)
        }

        match event {
            Event::Ate(key, grams, .. ) => {
                fn best_substitute_index(
                    study: &Shelf,
                    food_types: &FoodTypes,
                    key: &food::Key,
                    // TODO? offset param?
                ) -> ShelfIndex {
                    let Some(target_serving_size) =
                        food_types.iter().find(|f| &f.key == key).map(|f| f.serving) else {
                        return ShelfIndex(0);
                    };

                    // Favour items with a similar serving size, as a hueristic for similarity.
                    // TODO? Some kind of category system to define acceptable substitutes?

                    let mut best_index = 0;
                    let mut best_difference = Grams::MAX;

                    for i in 0..study.shelf.len() {
                        let candidate_key = &study.shelf[i].key;

                        for food in food_types {
                            if candidate_key == &food.key && &food.key != key {
                                let serving_size = food.serving;

                                let difference = target_serving_size.get().abs_diff(serving_size.get());

                                if difference < best_difference {
                                    best_difference = difference;
                                    best_index = i;
                                }

                                break
                            }
                        }
                    }

                    ShelfIndex(best_index)
                }

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
                        let food = study.shelf.swap_remove(index.0);

                        // Go check for more of the same thing
                        if let Some(new_index) = study.shelf.iter().position(|f| f.key == food.key) {
                            // TODO? track recursion depth so display can indent?
                            tracking_steps.push(TrackingStep::Ate {
                                eaten: food.grams,
                                key: food.key.clone(),
                                out_count: 0,
                                servings_count: calc_servings_count(food_types, &food.key, food.grams),
                            });

                            eat_at(study, tracking_steps, ShelfIndex(new_index), remaining_grams, food_types);

                            return
                        }

                        // Ran out; pick an alternate
                        tracking_steps.push(TrackingStep::Ate {
                            eaten: food.grams,
                            key: food.key.clone(),
                            out_count: remaining_grams,
                            servings_count: calc_servings_count(food_types, &food.key, food.grams),
                        });

                        study.perf.out_count += remaining_grams;

                        // TODO? Allow configuring this? Make random an option?
                        let substitute_index = best_substitute_index(study, food_types, &food.key);

                        eat_at(study, tracking_steps, substitute_index, remaining_grams, food_types);
                    }
                }

                if let Some(index) = study.shelf.iter().position(|f| f.key == key) {
                    eat_at(study, tracking_steps, ShelfIndex(index), grams, food_types);
                } else {
                    tracking_steps.push(TrackingStep::Ate {
                        eaten: 0,
                        key: key.clone(),
                        out_count: grams,
                        servings_count: 0.0,
                    });

                    study.perf.out_count += grams;

                    // TODO? Allow configuring this? Make random an option?
                    let substitute_index = best_substitute_index(study, food_types, &key);

                    eat_at(study, tracking_steps, substitute_index, grams, food_types);
                }
            },
            Event::Bought(food, minimum_purchase_servings) => {
                buy!(food, minimum_purchase_servings);
            }
            Event::BuyAllBasedOnFullness(
                BuyAllBasedOnFullnessParams {
                    max_count,
                    offset,
                    fullness_threshold,
                    minimum_purchase_servings,
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
                            let servings_per_pack: food::NonZeroServings = calc_servings_per_pack!(food, type_.serving);

                            total_fullness += food.current_fullness(minimum_purchase_servings, servings_per_pack);
                            break
                        }
                    }

                    if total_fullness < fullness_threshold && count < max_count {
                        buy!(Food::from_rng_of_type(&type_, rng), minimum_purchase_servings);
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
        Bought(Food, food::Servings),
        BuyAllBasedOnFullness(BuyAllBasedOnFullnessParams)
    }

    impl Event {
        fn from_rng(food_types: &FoodTypes, rng: &mut Xs) -> Self {
            match xs::range(rng, 0..2 as u32) {
                1 => Self::Bought(Food::from_rng(food_types, rng), 0),
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

        let day_count_min = spec.day_count_min as u32;
        let day_count_one_past_max = spec.day_count_one_past_max as u32;

        let day_count = xs::range(&mut rng, day_count_min..day_count_one_past_max) as usize;

        type Events = Vec<EventEntry>;

        let mut events: Events = Vec::with_capacity(day_count);

        struct EventSourceBundle<'rng, 'food_types, F>
        where
            F: FnMut(Event)
        {
            push_event: F,
            rng: &'rng mut Xs,
            food_types: &'food_types FoodTypes,
            recently_eaten_foods: Vec<food::Key>,
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
                minimum_purchase_servings: 0,
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
                push_event(Event::Bought(
                    Food::from_rng_of_type(&food_types[index], rng),
                    0,
                ));
            }
        }

        // TODO more realistic hunger model with meals

        // TODO hunger model, or modifier over all hunger models that tracks what was eaten recently and avoids 
        //      eating that for a while. Maybe pass down a filtered food_types?

        // TODO? Label certain foods as once-per-day? Or maybe some generalization of that, like n times per period

        // TODO purchase model that buys enough that we have n servings of everything

        fn fixed_servings_amount<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                rng,
                food_types: full_food_types,
                recently_eaten_foods,
                ..
            }: EventSourceBundle<F>,
            FixedServingsAmountParams { servings_per_day }: &FixedServingsAmountParams,
        ) {
            let food_types = 
                full_food_types.iter()
                    .filter(|t| !recently_eaten_foods.contains(&t.key))
                    .collect::<Vec<_>>()
                ;


            let mut g_state = xs::GaussianState::default();

            let mut servings_remaining = (*servings_per_day) as f32;
            while servings_remaining > 0. {
                let index = xs::range(rng, 0..food_types.len() as u32) as usize;

                let type_ = &food_types[index];

                // At least one serving, and as much as four.
                let servings_count = 1. + xs::gaussian_zero_to_one(rng, &mut g_state) * 3.;

                let serving_grams = type_.serving.get();

                let amount =
                    (servings_count * serving_grams as f32) as food::Grams;

                push_event(Event::Ate(
                    type_.key.clone(),
                    amount,
                ));

                servings_remaining -= servings_count;
            }
        }

        fn eat_exactly<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                ..
            }: EventSourceBundle<F>,
            EatExactlyParams { key_to_eat, grams_to_eat, }: &EatExactlyParams,
        ) {
            push_event(Event::Ate(
                key_to_eat.clone(),
                *grams_to_eat,
            ));
        }

        fn buy_exactly<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                ..
            }: EventSourceBundle<F>,
            BuyExactlyParams { key_to_buy, grams_to_buy, }: &BuyExactlyParams,
        ) {
            push_event(Event::Bought(
                Food::of_key(key_to_buy.clone(), food::Option{ grams: *grams_to_buy }),
                0,
            ));
        }

        fn fixed_hunger_amount<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                rng,
                food_types: full_food_types,
                recently_eaten_foods,
                ..
            }: EventSourceBundle<F>,
            FixedHungerAmountParams { grams_per_day }: &FixedHungerAmountParams,
        ) {
            let food_types = 
                full_food_types.iter()
                    .filter(|t| !recently_eaten_foods.contains(&t.key))
                    .collect::<Vec<_>>()
                ;

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

        fn buy_n_of_everything<F: FnMut(Event)>(
            EventSourceBundle {
                mut push_event,
                food_types,
                ..
            }: EventSourceBundle<F>,
            BuyNOfEverythingParams {
                n,
            }: &BuyNOfEverythingParams,
        ) {
            for type_ in food_types {
                for _ in 0..*n {
                    push_event(Event::Bought(
                        Food::of_type(type_, type_.options[0].clone()),
                        0,
                    ));
                }
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
                        push_event(Event::Bought(
                            Food::from_rng(food_types, rng),
                            0,
                        ));
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
            () => ({
                let mut start_index = events.len().saturating_sub(1);

                let mut day_marker_count = 0;

                while start_index > 0 && day_marker_count < 3 {
                    if matches!(
                        events[start_index],
                        EventEntry::InitialDayMarker | EventEntry::DayMarker,
                    ) {
                        day_marker_count += 1;
                    }
                    start_index -= 1;
                }

                let yesterday_slice = &events[start_index..];

                let mut recently_eaten_foods = Vec::with_capacity(yesterday_slice.len() / 2);

                for event_entry in yesterday_slice {
                    match event_entry {
                        EventEntry::Event(Event::Ate(key, _)) => {
                            recently_eaten_foods.push(key.to_owned());
                        }
                        EventEntry::Event(Event::Bought(..)) | EventEntry::Event(Event::BuyAllBasedOnFullness(..)) => {}
                        EventEntry::InitialDayMarker | EventEntry::DayMarker => {
                            break
                        }
                    }
                }

                EventSourceBundle {
                    push_event: |e| events.push(EventEntry::Event(e)),
                    rng: &mut rng,
                    food_types: &food_types,
                    recently_eaten_foods,
                }
            })
        }

        macro_rules! get_events {
            ($es_specs: expr, $i: expr) => {
                let i = $i;
                let chunk_index = i / 7;
                let bit_index = i % 7;

                for es_spec in $es_specs.iter() {
                    let happens_today = {
                        // TODO? Avoid needing to loop over the same chunks each day?
                        //       Or is nth on cycle already optimized?
                        match es_spec.recurrence.iter().cycle().nth(chunk_index) {
                            // Must be an empty list. That means always.
                            None => { true },
                            Some(chunk) => { ((chunk >> bit_index) & 1) == 1 },
                        }
                    };

                    if happens_today {
                        match &es_spec.kind {
                            EventSourceSpecKind::BuyIfBelowThreshold(p) => buy_if_below_threshold(b!(), &p),
                            EventSourceSpecKind::BuyIfHalfEmpty(p) => buy_if_half_empty(b!(), &p),
                            EventSourceSpecKind::BuyRandomVariety(p) => buy_random_variety(b!(), &p),
                            EventSourceSpecKind::BuyNOfEverything(p) => buy_n_of_everything(b!(), &p),
                            EventSourceSpecKind::BuyExactly(p) => buy_exactly(b!(), &p),
                            EventSourceSpecKind::EatExactly(p) => eat_exactly(b!(), &p),
                            EventSourceSpecKind::FixedHungerAmount(p) => fixed_hunger_amount(b!(), &p),
                            EventSourceSpecKind::FixedServingsAmount(p) => fixed_servings_amount(b!(), &p),
                            EventSourceSpecKind::ShopSomeDays(p) => shop_some_days(b!(), &p),
                            EventSourceSpecKind::RandomEvent(p) => random_event(b!(), &p),
                        }
                    }
                }
            }
        }

        get_events!(initial_event_source_specs, 0);

        events.push(EventEntry::InitialDayMarker);

        for i in 0..day_count {
            get_events!(repeated_event_source_specs, i);

            events.push(EventEntry::DayMarker);
        }
        assert!(events.len() > food_types.len());

        let mut all_stats = Vec::with_capacity(events.len() + 1);

        let mut tracking_steps = Vec::with_capacity(16);

        let mut day_number = 0;

        let mut daily_ate_total = 0;
        let mut daily_bought_total = 0;

        let event_count = events.len();

        for (i, event_entry) in events.drain(..).enumerate() {
            match event_entry {
                EventEntry::InitialDayMarker => {
                    if spec.show_step_by_step {
                        writeln!(w, "======= Start of the First Day ==========")?;
                        writeln!(w, "Day {day_number}")?;
                        daily_ate_total = 0;
                        daily_bought_total = 0;
                    }
                }
                EventEntry::DayMarker => {
                    day_number += 1;
                    if spec.show_step_by_step {
                        writeln!(w, "============= End of Day ================")?;
                        
                        writeln!(w, "Ate: {daily_ate_total}")?;
                        writeln!(w, "Bought: {daily_bought_total}")?;
                        writeln!(w, "Stock:")?;

                        // TODO? sort display of items? Or should the shelf data structure just be ordered?
                        for item in &study.shelf {
                            writeln!(w, "    {}: {}g", item.key, item.grams)?;
                        }

                        writeln!(w, "=========================================")?;
                        if i < event_count - 1 {
                            writeln!(w, "Day {day_number}")?;
                        }

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
    use crate::types::{BasicExtras, EventSourceSpecKind, BasicMode, Target};

    use std::io::Write;

    let spec: Spec = config::get_spec()?;

    let output = std::io::stdout();

    match spec.mode {
        Minimal => {
            minimal::run(&spec, &output)?;
        }
        Basic(ref extras) => {
            let dummy_output = DummyWrite {};

            type BIBTFn = Box<dyn Fn([f32; 1]) -> f32>;

            macro_rules! b_i_b_t_func_expr {
                (| $x: ident, $params: ident | $code: block) => ({
                    let spec = spec.clone();
                    let extras = extras.clone();
                    Box::new(move |[$x]: [f32; 1]| {
                        let mut repeated_event_source_specs = extras.repeated_event_source_specs.clone();

                        for ess in &mut repeated_event_source_specs {
                            match &mut ess.kind {
                                EventSourceSpecKind::BuyIfBelowThreshold($params) => {
                                    $code
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
                    })
                })
            }

            // TODO A way to do several runs in a row, with the seeds being chained together.
            //     Compute average performance, etc.
            // TODO A mode or some other way to describe in words the purchase strategy being used.
            //      This is expected to assist in actually applying it in real life, and also as a 
            //      measure of complexity.
            match extras.mode {
                BasicMode::Run => {
                    basic::run(&spec, &output)?;
                },
                BasicMode::PrintCalls(PrintCallsSpec{
                    target,
                    length,
                    offset,
                    step,
                }) => {
                    let func: BIBTFn = match target {
                        // TODO add more variants when we have more desired targets
                        Target::BuyIfBelowThresholdFullnessThreshold =>
                            b_i_b_t_func_expr!(
                                |x, params| {
                                    params.fullness_threshold = x;
                                }
                            ),
                        Target::BuyIfBelowThresholdMinimumPurchaseServings =>
                            b_i_b_t_func_expr!(
                                |x, params| {
                                    params.minimum_purchase_servings = x as _;
                                }
                            ),
                    };

                    let mut x = offset;
                    let end = offset + length;

                    writeln!(&output, "[")?;
                    while x <= end {
                        let y: f32 = func([x]);

                        writeln!(&output, "    ({x}, {y}),")?;

                        x += step;
                    }
                    writeln!(&output, "]")?;
                },
                BasicMode::Search(SearchSpec {
                    ref target,
                    length,
                    offset,
                }) => {
                    use minimize::{Call, minimize, regular_simplex_centered_at};

                    let center_1d = [ offset + length ];

                    let (func, center, label): (BIBTFn, [f32; 1], &str) = match target {
                        // TODO add more variants when we have more desired targets
                        Target::BuyIfBelowThresholdFullnessThreshold =>
                            (
                                b_i_b_t_func_expr!(
                                    |x, params| {
                                        params.fullness_threshold = x;
                                    }
                                ),
                                center_1d,
                                "fullness_threshold",
                            ),
                        Target::BuyIfBelowThresholdMinimumPurchaseServings =>
                            (
                                b_i_b_t_func_expr!(
                                    |x, params| {
                                        params.minimum_purchase_servings = x as _;
                                    }
                                ),
                                center_1d,
                                "minimum_purchase_servings",
                            ),
                    };

                    let simplex = regular_simplex_centered_at(length, center);

                    writeln!(&output, "simplex: {simplex:#?},")?;

                    let Call { xs: [x_1], y: performance } = minimize(
                        func,
                        simplex,
                        100,
                    );

                    writeln!(&output, "{label}: {x_1},")?;
                    writeln!(&output, "performance (closer to 0 is better): {performance},")?;
                },
            }
        }
    }

    Ok(())
}