use vec1::Vec1;

use std::num::NonZeroU8;

pub type FoodTypes = Vec1<food::Type>; 

pub struct FixedHungerAmountParams {
    pub grams_per_day: food::Grams,
}

/// One past max value of a die to roll from 0 to. So a value of 6 indicates a roll between 6 values from
/// 0 to 5 inclusive. Often used where somthing happens on a roll of 0, and nothing otherwise.
// TODO a more intuitive representation of the roll being made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct RollOnePastMax(NonZeroU8);

impl Default for RollOnePastMax {
    fn default() -> Self {
        Self(NonZeroU8::MIN)
    }
}

impl RollOnePastMax {
    pub fn u32(self) -> u32 {
        self.0.get() as _
    }
}

pub struct ShopSomeDaysParams {
    pub buy_count: u8,
    pub roll_one_past_max: RollOnePastMax,
}

pub struct RandomEventParams {
    pub roll_one_past_max: RollOnePastMax,
}

pub enum EventSourceSpec {
    FixedHungerAmount(FixedHungerAmountParams),
    ShopSomeDays(ShopSomeDaysParams),
    RandomEvent(RandomEventParams),
}

pub struct BasicExtras {
    pub food_types: FoodTypes,
    pub event_source_specs: Vec1<EventSourceSpec>,
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