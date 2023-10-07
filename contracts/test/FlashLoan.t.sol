import "forge-std/Test.sol";
import "forge-std/console.sol";
import "../src/FlashLoan.sol";

contract FlashLoanTest is Test {
    uint256 mainnetFork;
    address constant USDC_TOKEN = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;

    
    function setUp() public {
        mainnetFork = vm.createFork(vm.envString("MAINNET_PROVIDER_URL"), 18254502);
    }

    function testFork() public {
        vm.selectFork(mainnetFork);
        assertEq(vm.activeFork(), mainnetFork);
        // prank as vitalik address
        vm.startPrank(0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045);
        FlashLoan flashLoan = new FlashLoan();
        console.logString("I am here");
        console.logUint(flashLoan.checkBalance());
        vm.startPrank(0x5041ed759Dd4aFc3a72b8192C143F72f4724081A);
        IERC20(USDC_TOKEN).transfer(address(flashLoan), 1_000_000_000_000);
        flashLoan.flashLoan(0, 1_000_000_000, abi.encode(0, 1_000_000_000, 500));
    }

}