use alloy::{
    primitives::{Address, address},
    providers::Provider,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::Protocol;
use plutus_evm::EVM;
use std::sync::Arc;

use alloy::{providers::ProviderBuilder, rpc::client::ClientBuilder};
use dotenvy_macro::dotenv;
use plutus_defi_protocols_protocol::ProtocolFactory;
use plutus_defi_protocols_uniswap::v2::{UniswapV2Protocol, factory::UniswapV2Factory};

pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = Arc::new(
        ProviderBuilder::new().with_recommended_fillers().on_client(
            ClientBuilder::default()
                .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                .await?
                .boxed(),
        ),
    );

    let uniswapv2 = uniswap_v2_protocol(provider.clone()).await?;

    let mut evm = EVM::new(provider.clone()).await?;

    let usdt = ERC20::new(
        address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
        &mut evm,
    )?;
    let weth = ERC20::new(
        address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
        &mut evm,
    )?;

    let usdc = ERC20::new(USDC, &mut evm)?;

    let pools = uniswapv2.get_pools(usdc.address, weth.address, &mut evm)?;

    let mut pool = pools[0].clone();

    println!("{}", pool.address());

    let out = pool.simulate_swap(weth.address, weth.to_token_amount(1.0), &mut evm);
    let out = usdc.to_float_amount(out);

    println!("{out}");

    Ok(())
}

async fn uniswap_v2_protocol<P: Provider + 'static>(
    provider: P,
) -> anyhow::Result<UniswapV2Protocol> {
    Ok(<UniswapV2Protocol as ProtocolFactory<P>>::new(provider.get_chain_id().await?).unwrap())
}
