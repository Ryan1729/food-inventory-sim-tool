use crate::types::{self, food, BasicExtras, FoodTypes, Mode, Res, RollOnePastMax, Seed, Spec};
use std::collections::HashSet;

xflags::xflags! {
    cmd args {
        /// Path to a config file
        optional --file file: String
    }
}

struct DuplicateKeyError(types::food::Key);

impl core::fmt::Display for DuplicateKeyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Duplicate Food Type Key Found: {}", self.0)
    }
}

impl core::fmt::Debug for DuplicateKeyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for DuplicateKeyError {}

struct ExcessDataError {
    mode: RawMode,
    key_name: String,
}

impl core::fmt::Display for ExcessDataError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Excess key \"{}\" found for mode: {}", self.key_name, self.mode)
    }
}

impl core::fmt::Debug for ExcessDataError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for ExcessDataError {}

struct AtLeastOneRequiredError {
    mode: RawMode,
    key_name: String,
}

impl core::fmt::Display for AtLeastOneRequiredError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "At least one entry required for \"{}\" in mode: {}", self.key_name, self.mode)
    }
}

impl core::fmt::Debug for AtLeastOneRequiredError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for AtLeastOneRequiredError {}

#[derive(Debug, serde::Deserialize)]
enum RawMode {
    Minimal,
    Basic,
}

impl core::fmt::Display for RawMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            Self::Minimal => "Minimal",
            Self::Basic => "Basic",
        })
    }
}

#[derive(Debug, serde::Deserialize)]
enum RawEventSourceSpecKind {
    FixedHungerAmount,
    ShopSomeDays,
    RandomEvent,
}

#[derive(Debug, serde::Deserialize)]
struct RawEventSourceSpec {
    pub kind: RawEventSourceSpecKind,
    // All of the fields from all of the params
    #[serde(default)]
    pub grams_per_day: food::Grams,
    #[serde(default)]
    pub buy_count: u8,
    #[serde(default)]
    pub roll_one_past_max: RollOnePastMax,
}

#[derive(serde::Deserialize)]
struct RawSpec {
    // All modes
    pub mode: RawMode,
    pub seed: Option<Seed>,
    // Basic extras
    #[serde(default)]
    pub food_types: Vec<food::Type>,
    #[serde(default)]
    pub event_source_specs: Vec<RawEventSourceSpec>,
}

pub fn get_spec() -> Res<Spec> {
    let args = Args::from_env()?;

    let mut builder = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        ;

    if let Some(path) = args.file {
        builder = builder.add_source(config::File::with_name(&path))
    }

    builder = builder.add_source(
        config::Environment::with_prefix("FIST")
            .try_parsing(true)
            .list_separator(",")
            .with_list_parse_key("seed")
    );

    let unvalidated_spec = builder.build()?.try_deserialize::<RawSpec>()?;

    let mut spec = Spec::default();

    spec.seed = unvalidated_spec.seed;

    spec.mode = match &unvalidated_spec.mode {
        RawMode::Minimal => {
            if unvalidated_spec.food_types.len() > 0 {
                // TODO? A strict run mode that makes this a hard error?
                eprintln!(
                    "Warning: {}",
                    ExcessDataError{
                        mode: RawMode::Minimal,
                        key_name: "food_types".to_string(),
                    },
                );
            }

            Mode::Minimal
        },
        RawMode::Basic => {
            let food_types: FoodTypes = unvalidated_spec.food_types.try_into()?;

            let mut seen = HashSet::with_capacity(food_types.len());

            for food_type in food_types.iter() {
                if seen.contains(&food_type.key) {
                    return Err(Box::from(DuplicateKeyError(food_type.key.clone())));
                }

                seen.insert(food_type.key.clone());
            }

            let mut event_source_specs_vec = Vec::with_capacity(unvalidated_spec.event_source_specs.len());

            for i in 0..unvalidated_spec.event_source_specs.len() {
                use RawEventSourceSpecKind::*;
                use crate::types::EventSourceSpec as ESS;

                let e_s_spec = &unvalidated_spec.event_source_specs[i];

                fn is_default<T: PartialEq + Default>(thing: &T) -> bool {
                    PartialEq::eq(thing, &T::default())
                }

                macro_rules! excess_data_check {
                    ($($key: ident)+) => {
                        $(
                            if !is_default(&e_s_spec.$key) {
                                // TODO? A strict run mode that makes this a hard error?
                                eprintln!(
                                    "Warning: {}",
                                    ExcessDataError{
                                        mode: RawMode::Basic,
                                        key_name: format!("event_source_specs[{i}].{} for {:?}", stringify!($key), e_s_spec.kind),
                                    },
                                );
                            }
                        )+
                    }
                }

                use crate::types::{FixedHungerAmountParams, RandomEventParams, ShopSomeDaysParams};

                match e_s_spec.kind {
                    FixedHungerAmount => {
                        excess_data_check!(
                            buy_count roll_one_past_max
                        );

                        event_source_specs_vec.push(
                            ESS::FixedHungerAmount(FixedHungerAmountParams {
                                grams_per_day: e_s_spec.grams_per_day
                            })
                        );
                    },
                    ShopSomeDays => {
                        excess_data_check!(
                            grams_per_day
                        );

                        event_source_specs_vec.push(
                            ESS::ShopSomeDays(ShopSomeDaysParams {
                                buy_count: e_s_spec.buy_count,
                                roll_one_past_max: e_s_spec.roll_one_past_max,
                            })
                        );
                    },
                    RandomEvent => {
                        excess_data_check!(
                            grams_per_day buy_count
                        );

                        event_source_specs_vec.push(
                            ESS::RandomEvent(RandomEventParams {
                                roll_one_past_max: e_s_spec.roll_one_past_max,
                            })
                        );
                    },
                }
            }

            let event_source_specs = event_source_specs_vec
                .try_into()
                .map_err(
                    |_| AtLeastOneRequiredError {
                        mode: RawMode::Basic,
                        key_name: "event_source_specs".to_string(),
                    }
                )?;

            Mode::Basic(BasicExtras {
                food_types,
                event_source_specs,
            })
        },
    };

    Ok(spec)
}