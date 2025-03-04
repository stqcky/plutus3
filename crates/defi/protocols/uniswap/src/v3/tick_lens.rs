use ITickLens::PopulatedTick;
use alloy::{
    eips::BlockId,
    primitives::{Address, address},
    providers::Provider,
    sol,
};

sol!(
    #[sol(rpc)]
    contract ITickLens {
        struct PopulatedTick {
            int24 tick;
            int128 liquidity_net;
            uint128 liquidity_gross;
        }

        function getPopulatedTicksInWord(address pool, int16 tickBitmapIndex)
            public
            view
            override
            returns (PopulatedTick[] memory populatedTicks);
    }
);

const DEPLOYMENT_ADDRESS: Address = address!("0xbfd8137f7d1516D3ea5cA83523914859ec47F573");

pub struct TickLens;

impl TickLens {
    pub async fn get_populated_ticks_in_word<P: Provider>(
        pool: Address,
        tick_bitmap_index: i16,
        block: BlockId,
        provider: P,
    ) -> Result<Vec<PopulatedTick>, alloy::contract::Error> {
        Ok(ITickLens::new(DEPLOYMENT_ADDRESS, provider)
            .getPopulatedTicksInWord(pool, tick_bitmap_index)
            .block(block)
            .call()
            .await
            .map(|x| x.populatedTicks)?)
    }
}
