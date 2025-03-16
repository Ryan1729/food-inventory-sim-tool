struct GummyBear;

type Minimal = Option<GummyBear>;

fn main() {
    let mut study: Minimal = None;

    println!("{}", study.is_some());

    study = Some(GummyBear);

    println!("{}", study.is_some());

    study = None;

    println!("{}", study.is_some());
}
