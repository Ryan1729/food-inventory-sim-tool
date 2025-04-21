use vec1::Vec1;

use std::num::NonZeroU8;

pub type FoodTypes = Vec1<food::Type>; 

/// 64k items in one trip ought to be enough for anybody!
pub type ShoppingCount = u16;

pub type IndexOffset = usize;

pub type FullnessThreshold = f32;

#[derive(Clone, Debug)]
pub struct BuyAllBasedOnFullnessParams {
    pub max_count: ShoppingCount,
    pub offset: IndexOffset,
    pub fullness_threshold: FullnessThreshold,
}

#[derive(Clone, Debug)]
pub struct BuyIfHalfEmptyParams { 
    pub max_count: ShoppingCount,
    pub offset: IndexOffset,
}

#[derive(Clone, Debug)]
pub struct BuyRandomVarietyParams { 
    pub count: ShoppingCount,
    pub offset: IndexOffset,
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct ShopSomeDaysParams {
    pub buy_count: u8,
    pub roll_one_past_max: RollOnePastMax,
}

#[derive(Clone, Debug)]
pub struct RandomEventParams {
    pub roll_one_past_max: RollOnePastMax,
}

macro_rules! ess_def {
    (
        $($variant: ident ($params: ident) $(,)? )+
    ) => {
        #[derive(Clone, Debug)]
        pub enum EventSourceSpec {
            $( $variant($params), )+
        }

        #[derive(Debug, serde::Deserialize)]
        pub enum RawEventSourceSpecKind {
            $( $variant, )+
        }
    }
}

ess_def!{
    BuyIfBelowThreshold(BuyAllBasedOnFullnessParams),
    BuyIfHalfEmpty(BuyIfHalfEmptyParams),
    BuyRandomVariety(BuyRandomVarietyParams),
    FixedHungerAmount(FixedHungerAmountParams),
    ShopSomeDays(ShopSomeDaysParams),
    RandomEvent(RandomEventParams),
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub enum SearchTarget {
    #[default]
    BuyIfBelowThresholdFullnessThreshold
}

#[derive(Clone, Debug)]
pub enum BasicMode {
    Run,
    Search(SearchTarget),
}

#[derive(Clone, Debug)]
pub struct BasicExtras {
    pub mode: BasicMode,
    pub food_types: FoodTypes,
    pub initial_event_source_specs: Vec1<EventSourceSpec>,
    pub repeated_event_source_specs: Vec1<EventSourceSpec>,
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

    #[derive(Clone, Debug, serde::Deserialize)]
    pub struct Option {
        pub grams: Grams,
    }

    #[derive(Clone, Debug, serde::Deserialize)]
    pub struct Type {
        pub key: Key,
        pub options: Vec1<Option>,
    }
}

#[derive(Default)]
pub struct Spec {
    pub mode: Mode,
    pub seed: Option<Seed>,
    pub hide_summary: bool,
    pub show_grams: bool,
    pub show_items: bool,
    pub show_step_by_step: bool,
}

pub type Res<A> = Result<A, Box<dyn std::error::Error>>;