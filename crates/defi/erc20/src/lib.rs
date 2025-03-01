use IERC20::{IERC20Instance, decimalsCall, symbolCall};
use derive_more::Display;
use plutus_evm::{
    EVM,
    alloy::{contract, providers::Provider, sol, sol_types::SolCall as _},
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

    pub async fn new_with_provider<P: Provider>(
        address: Address,
        provider: P,
    ) -> Result<Self, plutus_evm::alloy::contract::Error> {
        let instance = IERC20Instance::new(address, provider);

        Ok(Self {
            address,
            symbol: instance.symbol().call().await?._0,
            decimals: instance.decimals().call().await?._0,
        })
    }

    pub fn to_token_amount(&self, amount: f64) -> U256 {
        U256::from(amount * 10f64.powi(self.decimals as i32))
    }

    pub fn to_float_amount(&self, amount: U256) -> f64 {
        f64::from(amount) / 10f64.powi(self.decimals as i32)
    }

    pub async fn balance_of<P: Provider>(
        &self,
        owner: Address,
        provider: P,
    ) -> Result<U256, contract::Error> {
        Ok(IERC20Instance::new(self.address, provider)
            .balanceOf(owner)
            .call()
            .await?
            ._0)
    }
}
