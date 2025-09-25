use alloy::sol;

sol!(
    #[sol(rpc)]
    contract RegenerativeVault {

        #[derive(Debug)]
        event Swap (
            bytes32 indexed poolId, 
            address indexed  tokenIn, 
            address indexed tokenOut, 
            uint256 amountIn, 
            uint256 amountOut
        );

    }
);
