use crate::types::{Spec, Res};

xflags::xflags! {
    cmd args {
        /// Path to a config file
        optional --file file: String
    }
}

pub fn get_spec() -> Res<Spec> {
    let args = Args::from_env()?;

    let mut builder = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        ;

    if let Some(path) = args.file {
        builder = builder.add_source(config::File::with_name(&path))
    }

    builder = builder.add_source(config::Environment::with_prefix("FIST"));

    Ok(builder.build()?.try_deserialize::<Spec>()?)
}