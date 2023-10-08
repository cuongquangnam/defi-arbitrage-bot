use defi_arbitrage_bot::regular_job;
use dotenv::dotenv;
use ethers_core::abi::Address;
use std::env;
use tokio::time::{sleep, Duration};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    dotenv().ok();

    loop {
        match regular_job::regular_job(
            env::var("MAINNET_PROVIDER_URL").unwrap(),
            env::var("FLASH_LOAN_ADDRESS").unwrap().parse::<Address>().unwrap(),
            env::var("PRIVATE_KEY").unwrap().try_into().unwrap(),
        )
        .await
        {
            Ok(receipt) => {
                println!("Flashed ");
                println!("Transaction hash is {:?}", receipt.transaction_hash);
            }
            // no arbitrage opportunity, wait for 15 mins and check again
            Err(_) => {}
        };
        // poll every 15 mins
        sleep(Duration::from_secs(15 * 60)).await;
    }
}
