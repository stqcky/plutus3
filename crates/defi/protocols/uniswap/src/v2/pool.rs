use IUniswapV2Pool::token0Call;
use alloy::{
    primitives::{Address, U256, aliases::U112},
    providers::Provider,
    sol,
    sol_types::SolCall,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::EVM;

sol!(
    #[sol(rpc)]
    contract IUniswapV2Pool {
        address public factory;
        address public token0;
        address public token1;

        uint112 private reserve0;
        uint112 private reserve1;

        function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    }
);

pub struct UniswapV2Pool {
    address: Address,

    token0: ERC20,
    token1: ERC20,

    reserves: Reserves,
}

impl UniswapV2Pool {
    pub fn new<P: Provider + std::fmt::Debug>(address: Address, evm: &mut EVM<P>) {
        evm.call(address, token0Call::new(()))
            .unwrap()
            .output
            .token0;
    }
}

pub struct Reserves(U112, U112);

impl LiquidityPool for UniswapV2Pool {
    fn simulate_swap(&self, token: Address, amount: U256) -> U256 {
        todo!()
    }
}
