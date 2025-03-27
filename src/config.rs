use crate::types::{self, food, BasicExtras, FoodTypes, Mode, Seed, Spec, Res};
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

#[derive(serde::Deserialize)]
struct RawSpec {
    // All modes
    pub mode: RawMode,
    pub seed: Option<Seed>,
    // Basic extras
    pub food_types: Vec<food::Type>,
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

            Mode::Basic(BasicExtras {
                food_types,
            })
        },
    };

    Ok(spec)
}