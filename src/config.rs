use crate::types::{self, food, BasicMode, BasicExtras, FixedServingsAmountParams, FoodTypes, Mode, PrintCallsSpec, RawEventSourceSpecKind, Res, RollOnePastMax, Seed, ServingsCount, Spec, Target};
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
struct RawEventSourceSpec {
    pub kind: RawEventSourceSpecKind,
    // All of the fields from all of the params
    #[serde(default)]
    pub grams_per_day: food::Grams,
    #[serde(default)]
    pub servings_per_day: ServingsCount,
    #[serde(default)]
    pub buy_count: u8,
    #[serde(default)]
    pub count: u16,
    #[serde(default)]
    pub max_count: u16,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub roll_one_past_max: RollOnePastMax,
    #[serde(default)]
    pub fullness_threshold: types::FullnessThreshold,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub enum RawBasicMode {
    #[default]
    Run,
    Search,
    PrintCalls,
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
    pub initial_event_source_specs: Vec<RawEventSourceSpec>,
    #[serde(default)]
    pub repeated_event_source_specs: Vec<RawEventSourceSpec>,
    #[serde(default)]
    pub basic_mode: RawBasicMode,
    #[serde(default)]
    pub basic_target: Target,
    // Output Flags section
    // Designed such that all false is a good default.
    #[serde(default)]
    pub hide_summary: bool,
    #[serde(default)]
    pub show_grams: bool,
    #[serde(default)]
    pub show_items: bool,
    #[serde(default)]
    pub show_step_by_step: bool,
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

    macro_rules! assign {
        ($($field: ident)+) => {
            $( spec.$field = unvalidated_spec.$field; )+
        }
    }
    assign!(seed hide_summary show_grams show_items show_step_by_step);

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

            fn is_default<T: PartialEq + Default>(thing: &T) -> bool {
                PartialEq::eq(thing, &T::default())
            }

            macro_rules! excess_data_check {
                ($specs: ident [$i: ident] $error_key: literal : $($key: ident)+) => ({
                    // TODO reverse the meaning of the keys, so we don't need to
                    // update every old one when adding a new key.

                    $(
                        if !is_default(&$specs[$i].$key) {
                            // TODO? A strict run mode that makes this a hard error?
                            eprintln!(
                                "Warning: {}",
                                ExcessDataError{
                                    mode: RawMode::Basic,
                                    key_name: format!("{}[{}].{} for {:?}", $error_key, $i, stringify!($key), $specs[$i].kind),
                                },
                            );
                        }
                    )+
                })
            }

            macro_rules! validate_event_source_specs {
                ($error_key: literal : $specs: expr) => ({
                    let specs = &$specs;
                    let mut specs_vec = Vec::with_capacity(specs.len());

                    for i in 0..specs.len() {
                        use crate::types::{
                            EventSourceSpec as ESS,
                            RawEventSourceSpecKind::*
                        };

                        let e_s_spec = &specs[i];

                        use crate::types::{
                            FixedHungerAmountParams,
                            RandomEventParams,
                            ShopSomeDaysParams,
                            BuyRandomVarietyParams,
                            BuyIfHalfEmptyParams,
                            BuyAllBasedOnFullnessParams,
                        };

                        match e_s_spec.kind {
                            BuyIfBelowThreshold => {
                                excess_data_check!(
                                    specs[i] $error_key : grams_per_day buy_count roll_one_past_max count
                                );

                                specs_vec.push(
                                    ESS::BuyIfBelowThreshold(BuyAllBasedOnFullnessParams {
                                        max_count: e_s_spec.max_count,
                                        offset: e_s_spec.offset,
                                        fullness_threshold: e_s_spec.fullness_threshold,
                                    })
                                );
                            },
                            BuyIfHalfEmpty => {
                                excess_data_check!(
                                    specs[i] $error_key : grams_per_day buy_count roll_one_past_max count
                                );

                                specs_vec.push(
                                    ESS::BuyIfHalfEmpty(BuyIfHalfEmptyParams {
                                        max_count: e_s_spec.max_count,
                                        offset: e_s_spec.offset,
                                    })
                                );
                            },
                            BuyRandomVariety => {
                                excess_data_check!(
                                    specs[i] $error_key : grams_per_day buy_count roll_one_past_max max_count
                                );

                                specs_vec.push(
                                    ESS::BuyRandomVariety(BuyRandomVarietyParams {
                                        count: e_s_spec.count,
                                        offset: e_s_spec.offset,
                                    })
                                );
                            }
                            FixedHungerAmount => {
                                excess_data_check!(
                                    specs[i] $error_key : buy_count roll_one_past_max max_count
                                );

                                specs_vec.push(
                                    ESS::FixedHungerAmount(FixedHungerAmountParams {
                                        grams_per_day: e_s_spec.grams_per_day
                                    })
                                );
                            },
                            FixedServingsAmount => {
                                excess_data_check!(
                                    specs[i] $error_key : buy_count roll_one_past_max max_count
                                );

                                specs_vec.push(
                                    ESS::FixedServingsAmount(FixedServingsAmountParams {
                                        servings_per_day: e_s_spec.servings_per_day
                                    })
                                );
                            },
                            ShopSomeDays => {
                                excess_data_check!(
                                    specs[i] $error_key : grams_per_day
                                );

                                specs_vec.push(
                                    ESS::ShopSomeDays(ShopSomeDaysParams {
                                        buy_count: e_s_spec.buy_count,
                                        roll_one_past_max: e_s_spec.roll_one_past_max,
                                    })
                                );
                            },
                            RandomEvent => {
                                excess_data_check!(
                                    specs[i] $error_key : grams_per_day buy_count
                                );

                                specs_vec.push(
                                    ESS::RandomEvent(RandomEventParams {
                                        roll_one_past_max: e_s_spec.roll_one_past_max,
                                    })
                                );
                            },
                        }
                    }

                    specs_vec
                        .try_into()
                        .map_err(
                            |_| AtLeastOneRequiredError {
                                mode: RawMode::Basic,
                                key_name: $error_key.to_string(),
                            }
                        )?
                })
            }

            let initial_event_source_specs = validate_event_source_specs!(
                "initial_event_source_specs" : unvalidated_spec.initial_event_source_specs
            );
            let repeated_event_source_specs = validate_event_source_specs!(
                "repeated_event_source_specs" : unvalidated_spec.repeated_event_source_specs
            );

            match &unvalidated_spec.basic_mode {
                RawBasicMode::Run => {
                    Mode::Basic(BasicExtras {
                        mode: BasicMode::Run,
                        food_types,
                        initial_event_source_specs,
                        repeated_event_source_specs,
                    })
                },
                RawBasicMode::Search => {
                    Mode::Basic(BasicExtras {
                        mode: BasicMode::Search(unvalidated_spec.basic_target),
                        food_types,
                        initial_event_source_specs,
                        repeated_event_source_specs,
                    })
                },
                RawBasicMode::PrintCalls => {
                    Mode::Basic(BasicExtras {
                        mode: BasicMode::PrintCalls(PrintCallsSpec {
                            target: unvalidated_spec.basic_target
                        }),
                        food_types,
                        initial_event_source_specs,
                        repeated_event_source_specs,
                    })
                },
            }
        },
    };

    Ok(spec)
}