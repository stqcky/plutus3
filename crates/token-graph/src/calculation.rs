use std::{fmt::Display, sync::Arc};

use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::{
    alloy::{eips::BlockId, providers::Provider, uint},
    revm::primitives::{I256, U256},
};

use crate::Opportunity;

pub struct CalculatedOpportunityLeg<P: Provider> {
    pub token_in: ERC20,
    pub token_out: ERC20,
    pub pool: Arc<dyn LiquidityPool<P>>,

    pub amount_in: U256,
    pub amount_out: U256,
}

pub struct CalculatedOpportunity<P: Provider> {
    pub base_token: ERC20,
    pub profit: U256,

    pub legs: Vec<CalculatedOpportunityLeg<P>>,
}

impl<P: Provider> Display for CalculatedOpportunity<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "------------------")?;
        writeln!(
            f,
            "Opportunity: {} {}",
            self.base_token.to_float_amount(self.profit),
            self.base_token.symbol
        )?;
        writeln!(f, "------------------")?;

        for leg in &self.legs {
            writeln!(
                f,
                "{} {} -> {} {} @ {}",
                leg.token_in.to_float_amount(leg.amount_in),
                leg.token_in.symbol,
                leg.token_out.to_float_amount(leg.amount_out),
                leg.token_out.symbol,
                leg.pool.identifier()
            )?;
        }

        writeln!(f, "------------------")?;

        Ok(())
    }
}

pub async fn calculate_opportunity<P: Provider + Clone>(
    opportunity: Opportunity<P>,
    block: BlockId,
    provider: P,
) -> Option<CalculatedOpportunity<P>> {
    let first_token_decimals = opportunity[0].token0.decimals;

    let amount_in =
        optimize_profit(&opportunity, first_token_decimals, block, provider.clone()).await;

    // tracing::info!(
    //     "calculate_opportunity: {:?}\n{opportunity:#?}",
    //     now.elapsed()
    // );

    let mut legs = vec![];
    let mut amount = amount_in;

    for leg in opportunity {
        let amount_out = leg
            .pool
            .simulate_swap(leg.token0.address, amount, block, provider.clone())
            .await;

        legs.push(CalculatedOpportunityLeg {
            token_in: leg.token0,
            token_out: leg.token1,
            pool: leg.pool,
            amount_in: amount,
            amount_out,
        });

        amount = amount_out;
    }

    let profit = amount.saturating_sub(amount_in);

    if !profit.is_zero() {
        Some(CalculatedOpportunity {
            base_token: legs[0].token_in.to_owned(),
            profit,
            legs,
        })
    } else {
        None
    }
}

async fn optimize_profit<P: Provider + Clone>(
    opportunity: &Opportunity<P>,
    decimals: u8,
    block: BlockId,
    provider: P,
) -> U256 {
    let get_profit = async |x| {
        I256::from_raw(simulate_opportunity(opportunity, x, block, provider.clone()).await)
            - I256::from_raw(x)
    };

    let (locked0, locked1) = opportunity[0]
        .pool
        .tokens_locked(provider.clone())
        .await
        .unwrap();

    let zero_for_one = opportunity[0].pool.token0().address == opportunity[0].token0.address;

    let locked = if zero_for_one { locked0 } else { locked1 } / uint!(4_U256);

    let mut lower_bound = uint!(0_U256);
    let mut upper_bound = locked;

    let precision = U256::from(decimals);

    let max_iter = 50;

    let two = uint!(2_U256);
    let three = uint!(3_U256);

    for _ in 0..max_iter {
        let point_lower = lower_bound + (upper_bound - lower_bound) / three;
        let point_higher = upper_bound - (upper_bound - lower_bound) / three;

        if point_higher - point_lower <= precision {
            break;
        }

        let (lower_profit, upper_profit) =
            tokio::join!(get_profit(point_lower), get_profit(point_higher));
        // tracing::info!("get_profit {:?}", now.elapsed());

        // tracing::info!("{lower_profit:?} {upper_profit:?}");

        if lower_profit > upper_profit {
            upper_bound = point_higher;
        } else {
            lower_bound = point_lower;
        }
    }

    (lower_bound + upper_bound) / two
}

// async fn optimize_profit_f<P: Provider + Clone>(
//     opportunity: &mut Opportunity<P>,
//     decimals: u8,
//     block: BlockId,
//     provider: P,
// ) -> U256 {
//     let base = opportunity[0].token0.clone();
//
//     let mut get_profit = async |x| {
//         base.to_float_amount(
//             simulate_opportunity(
//                 opportunity,
//                 base.to_token_amount(x),
//                 block,
//                 provider.clone(),
//             )
//             .await,
//         ) - x
//     };
//
//     let mut lower_bound = 0.0;
//     let mut upper_bound = 1000.0;
//
//     let max_iter = 50;
//
//     for _ in 0..max_iter {
//         let middle = (lower_bound + upper_bound) / 2.0;
//
//         let lower_profit = get_profit(lower_bound + (middle - lower_bound) / 2.0).await;
//         let upper_profit = get_profit(middle + (upper_bound - middle) / 2.0).await;
//
//         if lower_profit > upper_profit {
//             upper_bound = middle;
//         } else {
//             lower_bound = middle;
//         }
//     }
//
//     base.to_token_amount((lower_bound + upper_bound) / 2.0)
// }

async fn simulate_opportunity<P: Provider + Clone>(
    opportunity: &Opportunity<P>,
    amount_in: U256,
    block: BlockId,
    provider: P,
) -> U256 {
    let mut amount = amount_in;

    for leg in opportunity {
        amount = leg
            .pool
            .simulate_swap(leg.token0.address, amount, block, provider.clone())
            .await;
    }

    amount
}
