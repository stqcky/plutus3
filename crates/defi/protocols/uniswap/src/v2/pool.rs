use IUniswapV2Pool::token0Call;
use alloy::{
    primitives::{Address, U256, aliases::U112},
    providers::Provider,
    sol,
    sol_types::SolCall,
    uint,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::{EVM, contract::SmartContract, smart_contract, storage::FromStorageValue};

sol!(
    #[sol(rpc)]
    contract IUniswapV2Pool {
        address public factory;

        function token0() public returns (address);
        function token1() public returns (address);

        uint112 private reserve0;
        uint112 private reserve1;

        function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    }
);

pub struct UniswapV2Pool {
    token0: Address,
    token1: Address,
    reserves: Reserves,
}

impl FromStorageValue for Reserves {
    fn from_storage_value(value: U256) -> Self {
        todo!()
    }
}

pub struct Reserves(U112, U112);

impl LiquidityPool for UniswapV2Pool {
    fn simulate_swap(&self, token: Address, amount: U256) -> U256 {
        todo!()
    }
}
