use alloy::sol;

sol!(
    #[sol(rpc)]
    contract Comet {

        #[derive(Debug)]
        event Supply(
            address indexed from, 
            address indexed dst, 
            uint amount
        );

    }
);
