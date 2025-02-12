use IERC20::{balanceOfCall, decimalsCall, symbolCall};
use derive_more::Display;
use plutus_evm::{
    EVM,
    alloy::{providers::Provider, sol, sol_types::SolCall as _},
    errors::EvmCallError,
    revm::primitives::{Address, U256},
};

sol!(
    #[sol(rpc)]
    contract IERC20 {
        function name() public view returns (string memory);
        function symbol() public view returns (string memory);
        function decimals() public view returns (uint8);
        function totalSupply() public view returns (uint256);
        function balanceOf(address account) public view returns (uint256);
        function transfer(address to, uint256 value) public returns (bool);
        function allowance(address owner, address spender) public view returns (uint256);
        function approve(address spender, uint256 value) public returns (bool);
        function transferFrom(address from, address to, uint256 value) public returns (bool);
    }
);

#[derive(Clone, Debug, Eq, Display)]
#[display("{symbol}")]
pub struct ERC20 {
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
}

impl PartialEq for ERC20 {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl std::hash::Hash for ERC20 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

impl ERC20 {
    pub fn new<P: Provider>(address: Address, evm: &mut EVM<P>) -> Result<Self, EvmCallError<P>> {
        Ok(Self {
            address,
            symbol: evm.call(address, symbolCall::new(()))?.output._0,
            decimals: evm.call(address, decimalsCall::new(()))?.output._0,
        })
    }

    pub fn to_token_amount(&self, amount: f64) -> U256 {
        U256::from(amount * 10f64.powi(self.decimals as i32))
    }

    pub fn to_float_amount(&self, amount: U256) -> f64 {
        f64::from(amount) / 10f64.powi(self.decimals as i32)
    }

    pub fn balance_of<P: Provider>(
        &self,
        owner: Address,
        evm: &mut EVM<P>,
    ) -> Result<U256, EvmCallError<P>> {
        Ok(evm
            .call(self.address, balanceOfCall::new((owner,)))?
            .output
            ._0)
    }
}
