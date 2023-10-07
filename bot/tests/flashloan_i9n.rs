use dotenv::dotenv;
use ethers::{core::utils::Anvil, signers::Signer};
use ethers_core::{types::TransactionRequest, utils::AnvilInstance};

use defi_arbitrage_bot::cronJob::cron_job;
use ethers::{
    middleware::SignerMiddleware,
    prelude::abigen,
    providers::Middleware,
    providers::{Http, Provider},
    signers::LocalWallet,
    types::{Address, U256},
};
use std::sync::Arc;

// async fn teardown(docker: &Docker, id: &String) {
//     docker
//         .remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() }))
//         .await
//         .expect("Failure");
// }

// return anvil instance so that it would not be deallocated later
async fn setup() -> Result<(String, Address, AnvilInstance), Box<dyn std::error::Error>> {
    dotenv().ok();

    // // prepare docker for testing
    // let docker = Docker::connect_with_local_defaults().expect("Havine an error connecting with local defaults");
    // let mut port_bindings = HashMap::new();
    // port_bindings.insert(
    //     "8545/tcp".to_string(),
    //     Some(vec![PortBinding { host_ip: None, host_port: Some(String::from("8080")) }]),
    // );

    // let empty = HashMap::<(), ()>::new();
    // let mut exposed_ports = HashMap::new();
    // exposed_ports.insert("8545/tcp", empty);

    // let alpine_config = Config {
    //     image: Some(IMAGE),
    //     tty: Some(true),
    //     host_config: Some(HostConfig { port_bindings: Some(port_bindings), ..Default::default() }),
    //     exposed_ports: Some(exposed_ports),
    //     ..Default::default()
    // };
    // let id = docker.create_container::<&str, &str>(None, alpine_config).await.expect("Error creating a container").id;
    // docker.start_container::<String>(&id, None).await.expect("Error creating a container");

    // // start anvil network, fork from mainnet
    // let start_anvil_exec = docker
    //     .create_exec(
    //         &id,
    //         CreateExecOptions {
    //             attach_stdout: Some(true),
    //             attach_stderr: Some(true),
    //             cmd: Some(vec![
    //                 "anvil",
    //                 "--host",
    //                 "0.0.0.0",
    //                 "--fork-url",
    //                 (*env::var("MAINNET_PROVIDER_URL").unwrap()).into(),
    //             ]),
    //             ..Default::default()
    //         },
    //     )
    //     .await
    //     .expect("Error executing")
    //     .id;
    // docker.start_exec(&start_anvil_exec, None).await.unwrap();

    // // sleep for an amount of time so that anvil network is available for calling
    // sleep(Duration::from_secs(30)).await;

    let anvil = Anvil::new().fork("https://rpc.ankr.com/eth").fork_block_number(18297087u64).spawn();

    // let provider = Provider::<Http>::try_from("http://localhost:8080")?;
    let provider = Provider::<Http>::try_from(anvil.endpoint())?;
    let wallet: LocalWallet = anvil.keys()[0].clone().into();

    let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet.with_chain_id(ethers::types::Chain::Mainnet)));
    assert_eq!(client.address(), "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse::<Address>()?);

    // send 10_000 WETH to our address
    abigen!(IERC20, "../contracts/out/ERC20/IERC20.sol/IERC20.json");
    const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
    const TOP_HOLDER_WETH_ADDRESS: &str = "0x57757E3D981446D585Af0D9Ae4d7DF6D64647806";
    let weth_contract = IERC20::new(WETH_ADDRESS.parse::<Address>()?, client.clone());
    assert_eq!(weth_contract.balance_of(client.address()).await.unwrap(), U256::from(0));

    provider.request::<_, Option<String>>("anvil_impersonateAccount", [TOP_HOLDER_WETH_ADDRESS]).await.unwrap();

    let tx = TransactionRequest::new()
        .from(TOP_HOLDER_WETH_ADDRESS.parse::<Address>().unwrap())
        .data(
            (weth_contract
                .transfer(client.address(), U256::from(10_000).checked_mul(U256::exp10(18)).unwrap())
                .calldata())
            .unwrap(),
        )
        .to(weth_contract.address());

    client.send_transaction(tx, None).await.unwrap().await.unwrap().unwrap();
    provider.request::<_, Option<String>>("anvil_stopImpersonatingAccount", [TOP_HOLDER_WETH_ADDRESS]).await.unwrap();

    assert_eq!(
        weth_contract.balance_of(client.address()).await.unwrap(),
        U256::from(10_000).checked_mul(U256::exp10(18)).unwrap()
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
    let usdc_address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse::<Address>().unwrap();

    weth_contract.approve(router_address, U256::max_value()).send().await.unwrap();
    assert_eq!(weth_contract.allowance(client.address(), router_address).await.unwrap(), U256::max_value());

    // make the price on uniswap less efficient by swapping too much weth in
    IV2_Swap_Contract
        .swap_exact_tokens_for_tokens(
            U256::from(10_000).checked_mul(U256::exp10(18)).unwrap(),
            U256::from(0),
            vec![weth_address, usdc_address],
            client.address(),
        )
        .send()
        .await
        .unwrap();
    let usdc_contract = IERC20::new(usdc_address, client.clone());
    println!("balance is {:?}", usdc_contract.balance_of(client.address()).await);
    Ok((anvil.endpoint(), flash_loan_contract.address(), anvil))
}

#[tokio::test]
async fn test_loop() {
    print!("Hello");

    let (endpoint, flash_loan_address, anvil) = setup().await.unwrap();
    cron_job(endpoint, true, flash_loan_address).await.unwrap()

    // teardown(&docker, &id).await;
}
