use ethers::{
    middleware::SignerMiddleware,
    prelude::abigen,
    providers::{Http, Middleware, Provider},
    signers::LocalWallet,
    types::{Address, U256, U512},
};
use ethers_core::rand::thread_rng;
use futures_util::Future;
use std::sync::Arc;

// https://en.wikipedia.org/wiki/Golden-section_search
pub async fn golden_section_search<F, Fut>(
    mut a: U256,
    mut b: U256,
    func: F,
    tolerance: U256,
    rpc_url: String,
    flash_loan_address: Address,
    wallet: LocalWallet,
    max_fee_per_gas: U256,
) -> U256
where
    F: Fn(U256, String, Address, LocalWallet, U256) -> Fut,
    Fut: Future<Output = U512>,
{
    let golden_ratio = U512::from(1618);
    let mut golden_distance: U256 = (b - a)
        .full_mul(U256::from(1000))
        .checked_div(golden_ratio)
        .unwrap()
        .try_into()
        .unwrap();

    let mut x1 = b - golden_distance;
    let mut x2 = a + golden_distance;

    while b - a > tolerance {
        println!("b and a are {:?} {:?}", b, a);
        if func(
            x1,
            rpc_url.clone(),
            flash_loan_address,
            wallet.clone(),
            max_fee_per_gas,
        )
        .await
            > func(
                x2,
                rpc_url.clone(),
                flash_loan_address,
                wallet.clone(),
                max_fee_per_gas,
            )
            .await
        {
            b = x2;
            x2 = x1;
            golden_distance = (b - a)
                .full_mul(U256::from(1000))
                .checked_div(golden_ratio)
                .unwrap()
                .try_into()
                .unwrap();
            x1 = b - golden_distance;
        } else {
            a = x1;
            x1 = x2;
            golden_distance = (b - a)
                .full_mul(U256::from(1000))
                .checked_div(golden_ratio)
                .unwrap()
                .try_into()
                .unwrap();
            x2 = a + golden_distance;
        }
    }

    (a + b).checked_div(U256::from(2)).unwrap()
}

#[tokio::test]
async fn test_golden_section_search() {
    async fn func(
        x: U256,
        _rpc_url: String,
        _flash_loan_address: Address,
        _wallet: LocalWallet,
        _max_fee_per_gas: U256,
    ) -> U512 {
        // 100 * x + 1_000_000 - x**2
        U512::from(x)
            .checked_mul(U512::from(100))
            .unwrap()
            .checked_add(U512::from(1_000_000))
            .unwrap()
            - x.full_mul(x)
    }
    let min = U256::from(10);
    let max = U256::from(100);
    let max_fee_per_gas = U256::from(0);
    assert_eq!(
        golden_section_search(
            min,
            max,
            func,
            U256::from(1),
            "".to_string(),
            Address::zero(),
            LocalWallet::new(&mut thread_rng()),
            max_fee_per_gas
        )
        .await,
        U256::from(50)
    );
}

// objective function to calculate the profit we can make from the flash loan
pub async fn objective_func_for_flash_loan(
    borrow_amount: U256,
    rpc_url: String,
    flash_loan_address: Address,
    wallet: LocalWallet,
    max_fee_per_gas: U256,
) -> U512 {
    abigen!(IERC20, "../contracts/out/ERC20/IERC20.sol/IERC20.json");
    abigen!(FlashLoan, "../contracts/out/FlashLoan.sol/FlashLoan.json");

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let client = SignerMiddleware::new(provider.clone(), wallet.clone());

    let flash_loan_contract =
        FlashLoan::new(flash_loan_address, Arc::new(client.clone()));

    let DAI_TOKEN_ADDRESS = "0x6B175474E89094C44Da98b954EedeAC495271d0F"
        .parse::<Address>()
        .unwrap();

    let data = ethers::abi::encode(&[
        ethers::abi::Token::Uint(U256::from(0)),
        ethers::abi::Token::Uint(borrow_amount),
        ethers::abi::Token::Uint(U256::from(500)),
        ethers::abi::Token::Address(DAI_TOKEN_ADDRESS),
    ]);
    let flash_call = flash_loan_contract.flash_loan(
        U256::from(0),
        borrow_amount,
        data.into(),
    );
    match flash_call.call().await {
        Ok(weth_balance_increase) => {
            let gas_unit_estimate = flash_call.estimate_gas().await.unwrap();
            let u512_gas_cost_estimate: U512 =
            // using checked_mul here is because the expectation that the gas cost in wei
            // is not larger than u256
                gas_unit_estimate.checked_mul(max_fee_per_gas).unwrap().into();
            let u512_weth_balance_increase: U512 = weth_balance_increase.into();
            // add 2**256 - 1 in case u512_weth_balance_increase < u512_gas_cost_estimate
            // 2**256 - 1 + 2**256 - 1 < 2*512 - 1, still within the range of U512 to handle
            let diff = u512_weth_balance_increase
                .checked_add(U512::from(2).pow(256.into()) - 1)
                .unwrap()
                .checked_sub(u512_gas_cost_estimate)
                .unwrap();
            println!("{:?}", diff);
            return diff;
        }
        // despite alrady eliminating the case where we can't pay back the pool
        // still, just return 0 in case something goes wrong as 0 has the lowest value
        Err(e) => {
            println!("{:?}", e);
            return U512::from(0);
        }
    };
}
