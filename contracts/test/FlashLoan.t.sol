import "forge-std/Test.sol";
import "forge-std/console.sol";
import "../src/FlashLoan.sol";
import "@openzeppelin/contracts/interfaces/IERC20.sol";
import "@uniswap/swap-router-contracts/contracts/interfaces/IV2SwapRouter.sol";


contract FlashLoanTest is Test {
    uint256 mainnetFork;
    address constant DAI_TOKEN = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address constant ROUTER_ADDRESS = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
    address constant WETH_TOKEN = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

    
    function setUp() public {
        // fork mainnet
        mainnetFork = vm.createFork(vm.envString("MAINNET_PROVIDER_URL"), 18297087);
    }

    function testFlashLoan() public {
        vm.selectFork(mainnetFork);
        assertEq(vm.activeFork(), mainnetFork);
        // prank as top dai address, and send dai to this smart contract address
        address TOP_DAI_HOLDER = 0x60FaAe176336dAb62e284Fe19B885B095d29fB7F;
        vm.startPrank(TOP_DAI_HOLDER);
        IERC20(DAI_TOKEN).transfer(address(this), 153_000_000e18);
        vm.stopPrank();
        assertEq(IERC20(DAI_TOKEN).balanceOf(address(this)), 153_000_000e18);
        address[] memory path = new address[](2);
        path[0] = DAI_TOKEN;
        path[1] = WETH_TOKEN;
        // swap 10_000_000 DAI for WETH, so WETH is now worth more DAI in the uniswap v2 pool
        IERC20(DAI_TOKEN).approve(ROUTER_ADDRESS, type(uint256).max);
        IV2SwapRouter(ROUTER_ADDRESS).swapExactTokensForTokens(1_000_000e18, 0, path, address(this));
        FlashLoan flashLoan = new FlashLoan();
        uint256 balanceWETHBeforeFlash = IERC20(WETH_TOKEN).balanceOf(address(flashLoan));
        uint256 gasLeftBeforeFlash = gasleft();
        // borrow 1 WETH to flashloan
        uint256 amountWETHIncrease = flashLoan.flashLoan(0, 1e18, abi.encode(0, 1e18, 500, DAI_TOKEN));
        uint256 gasLeftAfterFlash = gasleft();
        uint256 weiSpentOnGas = (gasLeftBeforeFlash - gasLeftAfterFlash) * 60 gwei;
        uint256 balanceWETHAfterFlash = IERC20(WETH_TOKEN).balanceOf(address(flashLoan));
        assertLt(weiSpentOnGas, balanceWETHAfterFlash - balanceWETHBeforeFlash);
        assertEq(amountWETHIncrease, balanceWETHAfterFlash - balanceWETHBeforeFlash);
    }

}