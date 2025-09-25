use alloy::sol;

sol!(
    #[sol(rpc)]
    contract StCelo {

        #[derive(Debug)]
        event Transfer (
             address indexed from,  
             address indexed to, 
             uint256 value
            );

    }
);
