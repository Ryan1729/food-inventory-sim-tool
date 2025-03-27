use vec1::Vec1;

pub type FoodTypes = Vec1<food::Type>; 

pub struct BasicExtras {
    pub food_types: FoodTypes,
}

#[derive(Default)]
pub enum Mode {
    #[default]
    Minimal,
    Basic(BasicExtras),
}

pub type Seed = [u8; 16];

pub mod food {
    use super::*;

    // 64k grams ought to be enough for anybody!
    pub type Grams = u16;

    pub type Key = String;

    #[derive(serde::Deserialize)]
    pub struct Option {
        pub grams: Grams,
    }

    #[derive(serde::Deserialize)]
    pub struct Type {
        pub key: Key,
        pub options: Vec1<Option>,
    }
}

#[derive(Default)]
pub struct Spec {
    pub mode: Mode,
    pub seed: Option<Seed>,
}

pub type Res<A> = Result<A, Box<dyn std::error::Error>>;