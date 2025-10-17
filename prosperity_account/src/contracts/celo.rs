use alloy::sol;

sol!(

    contract Celo {
          #[derive(Debug)]
        event Transfer (
             address indexed from,
             address indexed to,
             uint256 value
            );
    }

);
