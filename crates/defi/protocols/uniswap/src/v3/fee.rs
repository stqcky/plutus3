#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    Ord,
    PartialOrd,
    Hash,
    strum::Display,
    strum::FromRepr,
    strum::EnumIter,
)]
#[repr(u32)]
pub enum FeeAmount {
    Lowest = 100,
    Low = 500,
    Medium = 3_000,
    High = 10_000,
}
