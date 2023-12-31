use dotenv::dotenv;
use ethers::{core::utils::Anvil, signers::Signer};
use ethers_core::{types::TransactionRequest, utils::AnvilInstance};

use defi_arbitrage_bot::regular_job::regular_job;
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
abigen!(FlashLoan, "../contracts/out/FlashLoan.sol/FlashLoan.json");

// return anvil instance so that it would not be deallocated later
async fn setup() -> Result<
    (
        String,
        Address,
        AnvilInstance,
        LocalWallet,
        Address,
        IERC20<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
        FlashLoan<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    ),
    Box<dyn std::error::Error>,
> {
    dotenv().ok();

    // prepare a local fork of mainnet and wallet
    let anvil = Anvil::new()
        .fork("https://rpc.ankr.com/eth")
        .fork_block_number(18297087u64)
        .spawn();
    let provider = Provider::<Http>::try_from(anvil.endpoint())?;
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let wallet_with_chain_id =
        wallet.with_chain_id(ethers::types::Chain::Mainnet);
    let client = Arc::new(SignerMiddleware::new(
        provider.clone(),
        wallet_with_chain_id.clone(),
    ));
    assert_eq!(
        client.address(),
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse::<Address>()?
    );

    // send 153 million DAI to our address !!!!!
    const DAI_ADDRESS: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
    const TOP_HOLDER_DAI_TOKEN: &str =
        "0x60FaAe176336dAb62e284Fe19B885B095d29fB7F";
    let dai_contract =
        IERC20::new(DAI_ADDRESS.parse::<Address>()?, client.clone());
    assert_eq!(
        dai_contract.balance_of(client.address()).await.unwrap(),
        U256::from(0)
    );
    provider
        .request::<_, Option<String>>(
            "anvil_impersonateAccount",
            [TOP_HOLDER_DAI_TOKEN],
        )
        .await
        .unwrap();
    let tx = TransactionRequest::new()
        .from(TOP_HOLDER_DAI_TOKEN.parse::<Address>().unwrap())
        .data(
            (dai_contract
                .transfer(
                    client.address(),
                    U256::from(153_000_000)
                        .checked_mul(U256::exp10(18))
                        .unwrap(),
                )
                .calldata())
            .unwrap(),
        )
        .to(dai_contract.address());
    client.send_transaction(tx, None).await.unwrap().await.unwrap().unwrap();
    provider
        .request::<_, Option<String>>(
            "anvil_stopImpersonatingAccount",
            [TOP_HOLDER_DAI_TOKEN],
        )
        .await
        .unwrap();
    assert_eq!(
        dai_contract.balance_of(client.address()).await.unwrap(),
        U256::from(153_000_000).checked_mul(U256::exp10(18)).unwrap()
    );

    // deploy flash loan contract
    let flash_loan_contract =
        FlashLoan::deploy(client.clone(), ()).unwrap().send().await.unwrap();

    // make the price on uniswap less efficient by swapping too much dai (1_000_000 DAI) in
    abigen!(
        IV2SwapRouter,
        "../contracts/out/IV2SwapRouter.sol/IV2SwapRouter.json"
    );
    let router_address = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"
        .parse::<Address>()
        .unwrap();
    let IV2_Swap_Contract = IV2SwapRouter::new(router_address, client.clone());
    let weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        .parse::<Address>()
        .unwrap();
    dai_contract
        .approve(router_address, U256::max_value())
        .send()
        .await
        .unwrap();
    assert_eq!(
        dai_contract.allowance(client.address(), router_address).await.unwrap(),
        U256::max_value()
    );
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

    Ok((
        anvil.endpoint(),
        flash_loan_contract.address(),
        anvil,
        wallet_with_chain_id.clone(),
        client.address(),
        weth_contract,
        flash_loan_contract,
    ))
}

#[tokio::test]
async fn test_regular_job() {
    let (
        endpoint,
        flash_loan_address,
        anvil,
        wallet_with_chain_id,
        client_address,
        weth_contract,
        _,
    ) = setup().await.unwrap();
    let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap();
    let eth_balance_client_before_cron_job =
        provider.get_balance(client_address, None).await.unwrap();
    let weth_balance_flash_loan_before_cron_job =
        weth_contract.balance_of(flash_loan_address).await.unwrap();
    let tx_receipt =
        regular_job(endpoint, flash_loan_address, wallet_with_chain_id)
            .await
            .unwrap();
    let weth_balance_flash_loan_after_cron_job =
        weth_contract.balance_of(flash_loan_address).await.unwrap();
    let eth_balance_client_after_cron_job =
        provider.get_balance(client_address, None).await.unwrap();
    let eth_decrease_balance_client =
        eth_balance_client_before_cron_job - eth_balance_client_after_cron_job;
    let weth_increase_balance_flash_loan =
        weth_balance_flash_loan_after_cron_job
            - weth_balance_flash_loan_before_cron_job;
    println!(
        "Increase weth balance of flash loan address {:?}",
        weth_increase_balance_flash_loan
    );
    println!("Decrease balance of client {:?}", eth_decrease_balance_client);
    println!("Tx receipt is {:?}", tx_receipt.gas_used);
    println!("Tx gas price is {:?}", tx_receipt.effective_gas_price);

    // make sure we make profit after the flash loan
    let diff_balance =
        weth_increase_balance_flash_loan - eth_decrease_balance_client;
    // just extra sanity check, the above subtraction operation will revert if
    // weth_increase_balance_flash_loan < eth_decrease_balance_client
    assert_eq!(diff_balance > U256::from(0), true);
}
