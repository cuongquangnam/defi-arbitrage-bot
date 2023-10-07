use dotenv::dotenv;
use ethers::{
    middleware::SignerMiddleware,
    prelude::abigen,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, Bytes, U256},
};
use std::env;
use std::{str::FromStr, sync::Arc};
use tokio::time::{sleep, Duration};

pub async fn cron_job(
    rpc_url: String,
    test: bool,
    flash_loan_address: Address,
) -> Result<(), Box<dyn std::error::Error + 'static>> {
    dotenv().ok();

    abigen!(IERC20, "../contracts/out/ERC20/IERC20.sol/IERC20.json");
    abigen!(FlashLoan, "../contracts/out/FlashLoan.sol/FlashLoan.json");

    // setup the signer
    println!("Before initializing provider");
    println!("{}", rpc_url);
    let provider = Provider::<Http>::try_from(rpc_url)?;
    println!("After initializing provider");
    println!("{:?}", provider.get_chainid().await.unwrap());
    let wallet: LocalWallet =
        env::var("ANVIL_PRIVATE_KEY").unwrap().parse::<LocalWallet>()?.with_chain_id(ethers::types::Chain::Mainnet);
    let client = SignerMiddleware::new(provider.clone(), wallet.clone());

    println!("Before initialize flash_loan_contract");
    let flash_loan_contract = FlashLoan::new(flash_loan_address, Arc::new(client.clone()));
    println!("After initialize flash_loan_contract");

    // strategy is to borrow USDC, then use USDC to buy WETH, and then use this WETH to buy USDC, then return back
    // flashLoan.flashLoan(0, 1_000_000_000, abi.encode(0, 1_000_000_000, 500));

    if test {
        let data = ethers::abi::encode(&[
            ethers::abi::Token::Uint(U256::from(0)),
            ethers::abi::Token::Uint(U256::from(1_000_000_000i64)),
            ethers::abi::Token::Uint(U256::from(500)),
        ]);
        println!("Before flash loan");
        match flash_loan_contract.flash_loan(U256::from(0), U256::from(1_000_000_000i64), data.into()).call().await {
            Ok(value) => println!("Yes, can flash loan"),
            Err(e) => {
                println!("{:?}", e);
                return Err(Box::new(e));
            }
        };
        println!("After flash loan");
        sleep(Duration::from_secs(10)).await;
        Ok(())
    } else {
        loop {
            println!("Im successfully looping ...");
            match flash_loan_contract
                .flash_loan(U256::from(0), U256::from(0), Bytes::from_str("0x00").unwrap())
                .call()
                .await
            {
                Ok(value) => println!("Yes, can flash loan"),
                Err(e) => return Err(Box::new(e)),
            };
            sleep(Duration::from_secs(10)).await;
        }
    }
}
