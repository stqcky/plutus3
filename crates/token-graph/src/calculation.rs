use std::{fmt::Display, sync::Arc};

use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::{
    alloy::{eips::BlockId, providers::Provider, uint},
    revm::primitives::U256,
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

pub fn calculate_opportunity<P: Provider + Clone>(
    opportunity: Opportunity<P>,
    block: BlockId,
) -> Option<CalculatedOpportunity<P>> {
    let amount_in = quadratic_search(&opportunity, block, 10);

    let mut legs = vec![];
    let mut amount = amount_in;

    // TODO: most of this can be removed.
    // just amount_in might be fine.

    for leg in opportunity {
        let amount_out = leg.pool.simulate_swap(leg.token0.address, amount, block);

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

// fn optimize_profit<P: Provider>(
//     opportunity: &Opportunity<P>,
//     decimals: u8,
//     block: BlockId,
//     iters: i32,
// ) -> U256 {
//     let get_profit =
//         |x| I256::from_raw(simulate_opportunity(opportunity, x, block)) - I256::from_raw(x);
//
//     let (locked0, locked1) = opportunity[0].pool.tokens_locked();
//
//     let zero_for_one = opportunity[0].pool.token0().address == opportunity[0].token0.address;
//
//     let locked = if zero_for_one { locked0 } else { locked1 } / uint!(10_U256);
//
//     let mut lower_bound = uint!(1_U256);
//     let mut upper_bound = locked;
//
//     let precision = U256::from(decimals);
//
//     let max_iter = iters;
//
//     let two = uint!(2_U256);
//     let three = uint!(3_U256);
//
//     for i in 0..max_iter {
//         let point_lower = lower_bound + (upper_bound - lower_bound) / three;
//         let point_higher = upper_bound - (upper_bound - lower_bound) / three;
//
//         println!(
//             "{i}: {point_lower} {point_higher} {}",
//             point_higher - point_lower
//         );
//
//         if point_higher - point_lower <= precision {
//             break;
//         }
//
//         let (lower_profit, upper_profit) = (get_profit(point_lower), get_profit(point_higher));
//         // tracing::info!("get_profit {:?}", now.elapsed());
//
//         // tracing::info!("{lower_profit:?} {upper_profit:?}");
//
//         if lower_profit > upper_profit {
//             upper_bound = point_higher;
//         } else {
//             lower_bound = point_lower;
//         }
//     }
//
//     (lower_bound + upper_bound) / two
// }

fn quadratic_search<P: Provider + Clone>(
    opportunity: &Opportunity<P>,
    block: BlockId,
    iters: i32,
) -> U256 {
    let get_profit =
        |x: i128| simulate_opportunity(opportunity, U256::from(x), block).to::<i128>() - x;

    let (locked0, locked1) = opportunity[0].pool.tokens_locked();

    let zero_for_one = opportunity[0].pool.token0().address == opportunity[0].token0.address;

    let locked = if zero_for_one { locked0 } else { locked1 } / uint!(4_U256);

    let mut first: i128 = 1;
    let mut last: i128 = locked.to();

    for _ in 0..iters {
        let one_fourth = (last - first) / 4;

        let mid = (first + last) / 2;
        let p1 = first + one_fourth;
        let p2 = last - one_fourth;

        // println!("{i}: {p1} {p2} {}", p2 - p1);

        let (p1_profit, mid_profit, p2_profit) = (get_profit(p1), get_profit(mid), get_profit(p2));

        if p1_profit > mid_profit {
            last = p1;
        } else if mid_profit > p2_profit {
            last = mid;
            first = p1;
        } else if mid_profit > p1_profit {
            first = mid;
            last = p2;
        } else if p2_profit > mid_profit {
            first = p2;
        }
    }

    U256::from((first + last) / 2)
}

fn simulate_opportunity<P: Provider>(
    opportunity: &Opportunity<P>,
    amount_in: U256,
    block: BlockId,
) -> U256 {
    let mut amount = amount_in;

    for leg in opportunity {
        amount = leg.pool.simulate_swap(leg.token0.address, amount, block);
    }

    amount
}
