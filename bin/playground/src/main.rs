use alloy::{
    primitives::{Address, address},
    providers::Provider,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::Protocol;
use plutus_evm::EVM;
use std::{sync::Arc, time::Instant};

use alloy::{providers::ProviderBuilder, rpc::client::ClientBuilder};
use dotenvy_macro::dotenv;
use plutus_defi_protocols_protocol::ProtocolFactory;
use plutus_defi_protocols_uniswap::v2::{UniswapV2Protocol, factory::UniswapV2Factory};

pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");
pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");

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

    let block_number = provider.get_block_number().await?;
    let now = Instant::now();
    let mut evm = EVM::new(provider.clone(), block_number);
    println!("{:?}", now.elapsed());

    let uni = uniswap_v2_protocol(provider.clone()).await?;

    let v2_pool = address!("F64Dfe17C8b87F012FCf50FbDA1D62bfA148366a");

    let now = Instant::now();
    let pool = uni.create_pool(v2_pool, &mut evm)?;
    println!("{:?}", now.elapsed());

    let now = Instant::now();
    let pool = uni.create_pool(v2_pool, &mut evm)?;
    println!("{:?}", now.elapsed());

    // let now = Instant::now();
    // let pool = uni
    //     .create_pool_with_provider(v2_pool, provider.clone())
    //     .await?;
    // println!("{:?}", now.elapsed());
    //
    // let now = Instant::now();
    // let pool = uni
    //     .create_pool_with_provider(v2_pool, provider.clone())
    //     .await?;
    // println!("{:?}", now.elapsed());

    // let pools = uni.get_pools(USDC, WETH, &mut evm)?;
    // println!("{}", pools[0].address());
    //
    Ok(())
}

async fn uniswap_v2_protocol<P: Provider + 'static>(
    provider: P,
) -> anyhow::Result<UniswapV2Protocol> {
    Ok(<UniswapV2Protocol as ProtocolFactory<P>>::new(provider.get_chain_id().await?).unwrap())
}
