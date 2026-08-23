# Plutus V3

A cross-protocol on-chain atomic arbitrage system targeting decentralized exchanges on the Arbitrum blockchain.

Back in the day (before timeboost landed) this bot was able to hit opportunities at `N+2`, just running locally on my computer.

Some of it's features:
- Uniswap, PancakeSwap and SushiSwap (V2/V3 version of them) reimplemented in Rust
- In-memory smart contract storage slot management
- Token graph building and negative cycle detection
- Custom multicall-like smart contract for atomic execution
- Automatic discovery, filtering and caching of liquidity pools

At one point I realized that I needed more abstractions and polish to get this bot to the next level, so I began a rewrite, which you can see in [plutus4](https://github.com/stqcky/plutus4).
