#[derive(serde::Deserialize)]
pub enum Mode {
    Minimal,
}

#[derive(serde::Deserialize)]
pub struct Spec {
    pub mode: Mode,
}

pub type Res<A> = Result<A, Box<dyn std::error::Error>>;