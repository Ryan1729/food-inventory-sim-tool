mod minimal {
    use std::io::Write;

    struct GummyBear;

    type Minimal = Option<GummyBear>;

    pub fn run(mut w: impl Write) -> Result<(), std::io::Error> {
        let mut study: Minimal = None;

        writeln!(w, "{}", study.is_some())?;

        study = Some(GummyBear);

        writeln!(w, "{}", study.is_some())?;

        study = None;

        writeln!(w, "{}", study.is_some())?;

        Ok(())
    }
}

#[derive(serde::Deserialize)]
enum Mode {
    Minimal,
}

#[derive(serde::Deserialize)]
struct Spec {
    mode: Mode,
}

type Res<A> = Result<A, Box<dyn std::error::Error>>;

fn main() -> Res<()> {
    use Mode::*;
    let spec: Spec = get_spec()?;

    let output = std::io::stdout();

    match spec.mode {
        Minimal => {
            minimal::run(&output)?;
        }
    }

    Ok(())
}

fn get_spec() -> Res<Spec> {
    // TODO accept CLI args to indicate a config file to overwrite the default config
    Ok(config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::with_prefix("FIST"))
        .build()?
        .try_deserialize::<Spec>()?)
}