use alloy::primitives::Address;

#[derive(Clone, Copy)]
pub struct PoolRecord {
    pub address: Address,
    pub protocol: ProtocolType,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolType {
    UniswapV2 = 0,
    UniswapV3 = 1,
}

impl From<i32> for ProtocolType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::UniswapV2,
            1 => Self::UniswapV3,
            _ => panic!("unknown protocol type"),
        }
    }
}
