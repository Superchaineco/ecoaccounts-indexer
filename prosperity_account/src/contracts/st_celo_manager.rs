use alloy::sol;

sol!(
    contract StCeloManager {
                #[derive(Debug)]
            event VotesScheduled (
                address indexed group,
               uint256 amount
            );
    }
);
