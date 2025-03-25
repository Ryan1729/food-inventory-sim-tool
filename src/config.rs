use crate::types::{self, BasicExtras, Mode, Spec, Res};
use std::collections::HashSet;

xflags::xflags! {
    cmd args {
        /// Path to a config file
        optional --file file: String
    }
}

#[derive(Debug)]
struct DuplicateKeyError(types::food::Key);

impl core::fmt::Display for DuplicateKeyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Duplicate Food Type Key Found: {}", self.0)
    }
}

impl std::error::Error for DuplicateKeyError {}

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

    let unvalidated_spec = builder.build()?.try_deserialize::<Spec>()?;

    match &unvalidated_spec.mode {
        Mode::Minimal => {},
        Mode::Basic(BasicExtras { food_types }) => {
            let mut seen = HashSet::with_capacity(food_types.len());

            for food_type in food_types.iter() {
                if seen.contains(&food_type.key) {
                    return Err(Box::from(DuplicateKeyError(food_type.key.clone())));
                }

                seen.insert(food_type.key.clone());
            }
        },
    }

    Ok(unvalidated_spec)
}