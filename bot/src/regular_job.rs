use crate::objective_func;
use ethers::{
    middleware::SignerMiddleware,
    prelude::abigen,
    providers::{Http, Provider},
    signers::LocalWallet,
    types::{Address, U256},
};
use ethers_core::types::TransactionReceipt;
use std::sync::Arc;

pub async fn regular_job(
    rpc_url: String,
    flash_loan_address: Address,
    wallet: LocalWallet,
) -> Result<TransactionReceipt, Box<dyn std::error::Error + 'static>> {
    abigen!(IERC20, "../contracts/out/ERC20/IERC20.sol/IERC20.json");
    abigen!(FlashLoan, "../contracts/out/FlashLoan.sol/FlashLoan.json");

    println!("{}", rpc_url);
    let provider = Provider::<Http>::try_from(rpc_url.clone())?;
    let client = SignerMiddleware::new(provider.clone(), wallet.clone());

    let flash_loan_contract =
        FlashLoan::new(flash_loan_address, Arc::new(client.clone()));

    let DAI_TOKEN_ADDRESS = "0x6B175474E89094C44Da98b954EedeAC495271d0F"
        .parse::<Address>()
        .unwrap();

    // find the range lower_bound and upper_bound so at least the flash loan does not fail due to not being able to pay back the uniswap pool
    // lower bound is 10**15 / 10**18 = 0.001 WETH
    let mut lower_bound = U256::exp10(15);
    loop {
        let data = ethers::abi::encode(&[
            ethers::abi::Token::Uint(U256::from(0)),
            ethers::abi::Token::Uint(lower_bound),
            ethers::abi::Token::Uint(U256::from(500)),
            ethers::abi::Token::Address(DAI_TOKEN_ADDRESS),
        ]);
        let flash_call = flash_loan_contract.flash_loan(
            U256::from(0),
            lower_bound,
            data.into(),
        );
        match flash_call.call().await {
            Ok(_) => break,
            Err(_) => {
                lower_bound = lower_bound.checked_mul(U256::from(10)).unwrap();
            }
        }
    }

    // upper bound is 10**25 / 10**18 = 10**7 WETH > total market cap of weth
    let mut upper_bound = U256::exp10(25);
    loop {
        let data = ethers::abi::encode(&[
            ethers::abi::Token::Uint(U256::from(0)),
            ethers::abi::Token::Uint(upper_bound),
            ethers::abi::Token::Uint(U256::from(500)),
            ethers::abi::Token::Address(DAI_TOKEN_ADDRESS),
        ]);
        let flash_call = flash_loan_contract.flash_loan(
            U256::from(0),
            upper_bound,
            data.into(),
        );
        match flash_call.call().await {
            Ok(_) => break,
            Err(_) => {
                upper_bound = upper_bound.checked_div(U256::from(10)).unwrap();
            }
        }
    }
    let optimal_val = objective_func::golden_section_search(
        lower_bound,
        upper_bound,
        objective_func::objective_func_for_flash_loan,
        U256::from(100),
        rpc_url.clone(),
        flash_loan_address,
        wallet.clone(),
    )
    .await;
    let data = ethers::abi::encode(&[
        ethers::abi::Token::Uint(U256::from(0)),
        ethers::abi::Token::Uint(optimal_val),
        ethers::abi::Token::Uint(U256::from(500)),
        ethers::abi::Token::Address(DAI_TOKEN_ADDRESS),
    ]);
    let flash_call =
        flash_loan_contract.flash_loan(U256::from(0), optimal_val, data.into());
    match flash_call.call().await {
        Ok(weth_balance_increase) => {
            let gas_estimate = flash_call.estimate_gas().await.unwrap();
            if weth_balance_increase > gas_estimate {
                println!("Let's flash loan!!!");
                let tx_receipt =
                    flash_call.send().await.unwrap().await.unwrap().unwrap();
                return Ok(tx_receipt);
            } else {
                return Err("No profit, can't flash loan".into());
            }
        }
        Err(e) => return Err(Box::new(e)),
    };
}
