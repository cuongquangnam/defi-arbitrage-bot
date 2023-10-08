use dotenv::dotenv;
use ethers::{core::utils::Anvil, signers::Signer};
use ethers_core::{types::TransactionRequest, utils::AnvilInstance};

use defi_arbitrage_bot::cronJob::cron_job;
use ethers::{
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    prelude::{abigen, Wallet},
    providers::Middleware,
    providers::{Http, Provider},
    signers::LocalWallet,
    types::{Address, U256},
};
use std::sync::Arc;

abigen!(IERC20, "../contracts/out/ERC20/IERC20.sol/IERC20.json");

// return anvil instance so that it would not be deallocated later
async fn setup() -> Result<
    (
        String,
        Address,
        AnvilInstance,
        LocalWallet,
        Address,
        IERC20<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    ),
    Box<dyn std::error::Error>,
> {
    dotenv().ok();

    let anvil = Anvil::new().fork("https://rpc.ankr.com/eth").fork_block_number(18297087u64).spawn();

    let provider = Provider::<Http>::try_from(anvil.endpoint())?;
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let wallet_with_chain_id = wallet.with_chain_id(ethers::types::Chain::Mainnet);

    let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet_with_chain_id.clone()));
    assert_eq!(client.address(), "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse::<Address>()?);

    // send 10_000 WETH to our address
    const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
    const DAI_ADDRESS: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
    const TOP_HOLDER_DAI_TOKEN: &str = "0x60FaAe176336dAb62e284Fe19B885B095d29fB7F";
    let dai_contract = IERC20::new(DAI_ADDRESS.parse::<Address>()?, client.clone());
    assert_eq!(dai_contract.balance_of(client.address()).await.unwrap(), U256::from(0));

    provider.request::<_, Option<String>>("anvil_impersonateAccount", [TOP_HOLDER_DAI_TOKEN]).await.unwrap();

    let tx = TransactionRequest::new()
        .from(TOP_HOLDER_DAI_TOKEN.parse::<Address>().unwrap())
        .data(
            (dai_contract
                .transfer(client.address(), U256::from(153_000_000).checked_mul(U256::exp10(18)).unwrap())
                .calldata())
            .unwrap(),
        )
        .to(dai_contract.address());

    client.send_transaction(tx, None).await.unwrap().await.unwrap().unwrap();
    provider.request::<_, Option<String>>("anvil_stopImpersonatingAccount", [TOP_HOLDER_DAI_TOKEN]).await.unwrap();

    assert_eq!(
        dai_contract.balance_of(client.address()).await.unwrap(),
        U256::from(153_000_000).checked_mul(U256::exp10(18)).unwrap()
    );

    // deploy flash loan contract
    abigen!(FlashLoan, "../contracts/out/FlashLoan.sol/FlashLoan.json");

    let flash_loan_contract = FlashLoan::deploy(client.clone(), ()).unwrap().send().await.unwrap();
    abigen!(IV2SwapRouter, "../contracts/out/IV2SwapRouter.sol/IV2SwapRouter.json");
    abigen!(IV3SwapRouter, "../contracts/out/IV3SwapRouter.sol/IV3SwapRouter.json");
    let router_address = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45".parse::<Address>().unwrap();

    let IV2_Swap_Contract = IV2SwapRouter::new(router_address, client.clone());
    let IV3_Swap_Contract = IV3SwapRouter::new(router_address, client.clone());
    let weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>().unwrap();

    dai_contract.approve(router_address, U256::max_value()).send().await.unwrap();
    assert_eq!(dai_contract.allowance(client.address(), router_address).await.unwrap(), U256::max_value());

    // make the price on uniswap less efficient by swapping too much dai in
    IV2_Swap_Contract
        .swap_exact_tokens_for_tokens(
            U256::from(1_000_000).checked_mul(U256::exp10(18)).unwrap(),
            U256::from(0),
            vec![DAI_ADDRESS.parse::<Address>()?, weth_address],
            client.address(),
        )
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    let weth_contract = IERC20::new(weth_address, client.clone());
    println!("balance is {:?}", weth_contract.balance_of(client.address()).await);
    Ok((
        anvil.endpoint(),
        flash_loan_contract.address(),
        anvil,
        wallet_with_chain_id.clone(),
        client.address(),
        weth_contract,
    ))
}

#[tokio::test]
async fn test_loop() {
    let (endpoint, flash_loan_address, anvil, wallet_with_chain_id, client_address, weth_contract) =
        setup().await.unwrap();
    let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap();
    let balance_client_before_cron_job = provider.get_balance(client_address, None).await.unwrap();
    let balance_flash_loan_before_cron_job = weth_contract.balance_of(flash_loan_address).await.unwrap();
    cron_job(endpoint, true, flash_loan_address, wallet_with_chain_id).await.unwrap();
    let balance_flash_loan_after_cron_job = weth_contract.balance_of(flash_loan_address).await.unwrap();
    let balance_client_after_cron_job = provider.get_balance(client_address, None).await.unwrap();
    let decrease_balance_client = balance_client_before_cron_job - balance_client_after_cron_job;
    let increase_weth_balance_flash_loan = balance_flash_loan_after_cron_job - balance_flash_loan_before_cron_job;
    println!("Increase weth balance of flash loan address {:?}", increase_weth_balance_flash_loan);
    println!("Decrease balance of client {:?}", decrease_balance_client);
    assert_eq!(increase_weth_balance_flash_loan - decrease_balance_client > U256::from(0), true);
}
