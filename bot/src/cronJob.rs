use ethers::{
    middleware::SignerMiddleware,
    prelude::abigen,
    providers::{Http, Middleware, Provider},
    signers::LocalWallet,
    types::{Address, Bytes, U256},
};
use ethers_core::types::TransactionReceipt;
use std::{str::FromStr, sync::Arc};
use tokio::time::{sleep, Duration};

pub async fn cron_job(
    rpc_url: String,
    test: bool,
    flash_loan_address: Address,
    wallet: LocalWallet,
) -> Result<TransactionReceipt, Box<dyn std::error::Error + 'static>> {
    abigen!(IERC20, "../contracts/out/ERC20/IERC20.sol/IERC20.json");
    abigen!(FlashLoan, "../contracts/out/FlashLoan.sol/FlashLoan.json");

    // setup the signer
    println!("{}", rpc_url);
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = SignerMiddleware::new(provider.clone(), wallet.clone());

    let flash_loan_contract = FlashLoan::new(flash_loan_address, Arc::new(client.clone()));

    // strategy is to borrow USDC, then use USDC to buy WETH, and then use this WETH to buy USDC, then return back
    // flashLoan.flashLoan(0, 1_000_000_000, abi.encode(0, 1_000_000_000, 500));

    let DAI_TOKEN_ADDRESS = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse::<Address>().unwrap();

    let data = ethers::abi::encode(&[
        ethers::abi::Token::Uint(U256::from(0)),
        ethers::abi::Token::Uint(U256::from(U256::exp10(18))),
        ethers::abi::Token::Uint(U256::from(500)),
        ethers::abi::Token::Address(DAI_TOKEN_ADDRESS),
    ]);
    let flash_call = flash_loan_contract.flash_loan(U256::from(0), U256::exp10(18), data.into());
    match flash_call.call().await {
        Ok(weth_balance_increase) => {
            let gas_estimate = flash_call.estimate_gas().await.unwrap();
            if weth_balance_increase > gas_estimate {
                println!("Let's flash loan!!!");
                let tx_receipt = flash_call.send().await.unwrap().await.unwrap().unwrap();
                return Ok(tx_receipt);
            } else {
                return Err("No profit, can't flash loan".into());
            }
        }
        Err(e) => return Err(Box::new(e)),
    };
}
