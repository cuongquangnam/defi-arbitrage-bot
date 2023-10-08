pragma solidity ^0.8.0;

import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Pool.sol";
import "@uniswap/v3-core/contracts/interfaces/callback/IUniswapV3FlashCallback.sol";
import "@openzeppelin/contracts/interfaces/IERC20.sol";
import "@uniswap/swap-router-contracts/contracts/interfaces/IV2SwapRouter.sol";
import "@uniswap/swap-router-contracts/contracts/interfaces/IV3SwapRouter.sol";
// import "@uniswap/v2-periphery/contracts/libraries/UniswapV2Library.sol";
import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Factory.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

import "forge-std/console.sol";

error NOT_UNISWAP_V3_POOL_ADDRESS();
error FLASH_LOAN_NOT_SUCCESSFUL();

contract FlashLoan is IUniswapV3FlashCallback, Ownable { 
    address constant DAI_TOKEN = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address constant USDC_TOKEN = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant WETH_TOKEN = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant UNISWAP_V3_FACTORY = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    address immutable V3_WETH_USDC_ADDRESS;

    address SWAP_ROUTER_02_ADDRESS = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;

    constructor() Ownable() {
        V3_WETH_USDC_ADDRESS = IUniswapV3Factory(UNISWAP_V3_FACTORY).getPool(WETH_TOKEN, USDC_TOKEN, 500);
    }

    function flashLoan(uint256 amountUSDCToBorrow, uint256 amountWETHToBorrow, bytes calldata data) external onlyOwner returns (uint256 amountWETHIncrease) {
        uint256 balanceWETHBeforeFlash = IERC20(WETH_TOKEN).balanceOf(address(this));
        IUniswapV3Pool(V3_WETH_USDC_ADDRESS).flash(address(this), amountUSDCToBorrow, amountWETHToBorrow, data);
        uint256 balanceWETHAfterFlash = IERC20(WETH_TOKEN).balanceOf(address(this));
        amountWETHIncrease = balanceWETHAfterFlash - balanceWETHBeforeFlash;
    }  

    function uniswapV3FlashCallback(
        uint256,
        uint256 flashLoanFee,
        bytes calldata data
    ) external {
        // checking whether the address is as expected
        if (msg.sender!=V3_WETH_USDC_ADDRESS) revert NOT_UNISWAP_V3_POOL_ADDRESS();
        (, uint256 amountWETHBorrow , uint24 fee, address token_address) = abi.decode(
            data, 
            (uint256, uint256, uint24, address)
        );
        // swap WETH for token
        uint256 balanceTokenBefore1stSwap = IERC20(token_address).balanceOf(address(this));
        address[] memory path = new address[](2);
        path[0] = WETH_TOKEN;
        path[1] = token_address;
        IERC20(WETH_TOKEN).approve(SWAP_ROUTER_02_ADDRESS, amountWETHBorrow);
        IV2SwapRouter(SWAP_ROUTER_02_ADDRESS).swapExactTokensForTokens(amountWETHBorrow, 0, path, address(this));
        uint256 balanceTokenAfter1stSwap = IERC20(token_address).balanceOf(address(this));
        uint256 balanceTokenIncrease = balanceTokenAfter1stSwap - balanceTokenBefore1stSwap;
        uint256 balanceWETHAfter1stSwap = IERC20(WETH_TOKEN).balanceOf(address(this));
        IERC20(DAI_TOKEN).approve(SWAP_ROUTER_02_ADDRESS, balanceTokenIncrease);
        // swap token for WETH
        swapTokenForWETH(balanceTokenIncrease, fee, token_address);
        uint256 balanceWETHAfter2ndSwap = IERC20(WETH_TOKEN).balanceOf(address(this));
        uint256 balanceWETHIncrease = balanceWETHAfter2ndSwap - balanceWETHAfter1stSwap;
        if (balanceWETHIncrease >= amountWETHBorrow + flashLoanFee) 
            IERC20(WETH_TOKEN).transfer(msg.sender, amountWETHBorrow + flashLoanFee);
        else revert FLASH_LOAN_NOT_SUCCESSFUL();
    }

    function swapTokenForWETH(uint256 balanceTokenIncrease, uint24 fee, address token_address) internal {
        // swap token for WETH
        IV3SwapRouter.ExactInputSingleParams memory params =  IV3SwapRouter.ExactInputSingleParams ({
            tokenIn: token_address,
            tokenOut: WETH_TOKEN,
            fee: fee,
            recipient: address(this),
            amountIn: balanceTokenIncrease,
            amountOutMinimum: 0,
            sqrtPriceLimitX96: 0 
        });
        IV3SwapRouter(SWAP_ROUTER_02_ADDRESS).exactInputSingle(params);
    }

    function withdrawToken(address tokenAddress, uint256 amount) external onlyOwner {
        IERC20(tokenAddress).transfer(msg.sender, amount);
    }
}

