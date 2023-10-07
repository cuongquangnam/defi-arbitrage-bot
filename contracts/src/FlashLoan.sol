pragma solidity ^0.8.0;

import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Pool.sol";
import "@uniswap/v3-core/contracts/interfaces/callback/IUniswapV3FlashCallback.sol";
import "@openzeppelin/contracts/interfaces/IERC20.sol";
import "@uniswap/swap-router-contracts/contracts/interfaces/IV2SwapRouter.sol";
import "@uniswap/swap-router-contracts/contracts/interfaces/IV3SwapRouter.sol";
// import "@uniswap/v2-periphery/contracts/libraries/UniswapV2Library.sol";
import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Factory.sol";
import "forge-std/console.sol";

error NOT_UNISWAP_V3_POOL_ADDRESS();
error FLASH_LOAN_NOT_SUCCESSFUL();

contract FlashLoan is IUniswapV3FlashCallback { 
    address constant DAI_TOKEN = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address constant USDC_TOKEN = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant WETH_TOKEN = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant UNISWAP_V3_FACTORY = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    address immutable V3_DAI_USDC_ADDRESS;

    address SWAP_ROUTER_02_ADDRESS = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;

    constructor() {
        V3_DAI_USDC_ADDRESS = IUniswapV3Factory(UNISWAP_V3_FACTORY).getPool(DAI_TOKEN, USDC_TOKEN, 500);
    }

    function flashLoan(uint256 amount0, uint256 amount1, bytes calldata data) public {
        IUniswapV3Pool(V3_DAI_USDC_ADDRESS).flash(address(this), amount0, amount1, data);

    }  

    function uniswapV3FlashCallback(
        uint256,
        uint256 fee1,
        bytes calldata data
    ) external {
        // checking whether the address is as expected
        if (msg.sender!=V3_DAI_USDC_ADDRESS) revert NOT_UNISWAP_V3_POOL_ADDRESS();
        (, uint256 amount1, uint24 fee) = abi.decode(
            data, 
            (uint256, uint256, uint24)
        );
        // swap USDC for WETH
        uint256 balanceWETHBefore1stSwap = IERC20(WETH_TOKEN).balanceOf(address(this));
        address[] memory path = new address[](2);
        path[0] = USDC_TOKEN;
        path[1] = WETH_TOKEN;
        console.log(USDC_TOKEN);
        console.log(IERC20(USDC_TOKEN).balanceOf(address(this)));
        IERC20(USDC_TOKEN).approve(SWAP_ROUTER_02_ADDRESS, amount1);
        IV2SwapRouter(SWAP_ROUTER_02_ADDRESS).swapExactTokensForTokens(amount1, 0, path, address(this));
        uint256 balanceWETHAfter1stSwap = IERC20(WETH_TOKEN).balanceOf(address(this));
        uint256 balanceWETHIncrease = balanceWETHAfter1stSwap - balanceWETHBefore1stSwap;
        uint256 balanceUSDCAfter1stSwap = IERC20(USDC_TOKEN).balanceOf(address(this));
        IERC20(WETH_TOKEN).approve(SWAP_ROUTER_02_ADDRESS, balanceWETHIncrease);
        // swap WETH for USDC
        swapWETHForUSDC(balanceWETHIncrease, fee);
        uint256 balanceUSDCAfter2ndSwap = IERC20(USDC_TOKEN).balanceOf(address(this));
        uint256 balanceUSDCIncrease = balanceUSDCAfter2ndSwap - balanceUSDCAfter1stSwap;
        // transferBack(balanceUSDCIncrease, amount0, fee0);
        if (balanceUSDCIncrease >= amount1 + fee1) 
        IERC20(USDC_TOKEN).transfer(msg.sender, amount1 + fee1);
        else revert FLASH_LOAN_NOT_SUCCESSFUL();
    }

    function swapWETHForUSDC(uint256 balanceWETHIncrease, uint24 fee) internal {
        // swap WETH for USDC
        IV3SwapRouter.ExactInputSingleParams memory params =  IV3SwapRouter.ExactInputSingleParams ({
            tokenIn: WETH_TOKEN,
            tokenOut: USDC_TOKEN,
            fee: fee,
            recipient: address(this),
            amountIn: balanceWETHIncrease,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0 
        });
        IV3SwapRouter(SWAP_ROUTER_02_ADDRESS).exactInputSingle(params);
    }

    function checkBalance() external view returns (uint256) {
        return msg.sender.balance;
    }
}

