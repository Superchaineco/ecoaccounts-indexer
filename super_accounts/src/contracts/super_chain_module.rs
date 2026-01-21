use alloy::sol;

sol!(
    #[sol(rpc)]
    contract SuperChainModule {

        #[derive(Debug)]
        struct NounMetadata {
            uint48 background;
            uint48 body;
            uint48 accessory;
            uint48 head;
            uint48 glasses;
        }

        #[derive(Debug)]
        event SuperChainSmartAccountCreated(
        address indexed safe,
        address indexed initialOwner,
        string superChainId,
        NounMetadata noun
    );

    event OwnerAdded(
        address indexed safe,
        address indexed newOwner,
        string superChainId
    );
    }

);
