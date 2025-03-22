#[derive(serde::Deserialize)]
pub enum Mode {
    Minimal,
    Basic,
}

pub type Seed = [u8; 16];

#[derive(serde::Deserialize)]
pub struct Spec {
    pub mode: Mode,
    pub seed: Option<Seed>,
}

pub type Res<A> = Result<A, Box<dyn std::error::Error>>;