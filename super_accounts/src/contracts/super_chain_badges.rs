use alloy::sol;

sol!(
    #[sol(rpc)]
    contract SuperChainBadges {

        #[derive(Debug)]
    event BadgeTierUpdated(
    address indexed user,
    uint256 indexed badgeId,
    uint256 tier,
    uint256 points,
    string uri
);

        #[derive(Debug)]
        event BadgeMinted(
        address indexed user,
        uint256 indexed badgeId,
        uint256 initialTier,
        uint256 points,
        string uri
        );
    }
);
