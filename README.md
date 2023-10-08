## Defi Trading Bot using Foundry and Rust

This is a fun project to build trading bot using foundry and rust

### Directory structure

```
├── Cargo.toml
├── bot // <-- Logic for the bot
├── contracts // <- The smart contracts + tests using Foundry
```

### Intall and build 
(Note: should install and build first before running any test)

```
pnpm i
cd contracts && forge build
```

### Test smart contracts
```
cd contracts && forge test
```

### Test logic for the bot
```
cd bot && cargo test
```

### Run the bot

Create an .env file in the root directory of the repo

```
cd bot && cargo run
```
### Strategy

The main smart contract for this reppo is contracts/src/FlashLoan.sol.
The purpose is to allow the owner of the smart contract to flash loan WETH from uniswap and then arbitrage between WETH-TOKEN and TOKEN-WETH, where TOKEN here is the token address specified by the contract owner.

The current bot only tries to arbitrage between WETH and DAI, in the future, can consider adding support for between WETH and more tokens in the logic for the bot

