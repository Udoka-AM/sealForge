use zcash_protocol::{
    consensus::{BlockHeight, MainNetwork, NetworkType, NetworkUpgrade, Parameters, TestNetwork},
    local_consensus::LocalNetwork,
};

/// Consensus parameters for the networks sealForge runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainParams {
    Mainnet,
    Testnet,
    /// Local regtest chain (scripts/regtest.sh): every upgrade through Ironwood
    /// (NU6.3) active from height 1, NU7 off.
    Regtest(LocalNetwork),
}

impl ChainParams {
    pub fn regtest() -> Self {
        let one = Some(BlockHeight::from_u32(1));
        Self::Regtest(LocalNetwork {
            overwinter: one,
            sapling: one,
            blossom: one,
            heartwood: one,
            canopy: one,
            nu5: one,
            nu6: one,
            nu6_1: one,
            nu6_2: one,
            nu6_3: one,
            nu7: None,
        })
    }
}

impl Parameters for ChainParams {
    fn network_type(&self) -> NetworkType {
        match self {
            Self::Mainnet => NetworkType::Main,
            Self::Testnet => NetworkType::Test,
            Self::Regtest(_) => NetworkType::Regtest,
        }
    }

    fn activation_height(&self, nu: NetworkUpgrade) -> Option<BlockHeight> {
        match self {
            Self::Mainnet => MainNetwork.activation_height(nu),
            Self::Testnet => TestNetwork.activation_height(nu),
            Self::Regtest(local) => local.activation_height(nu),
        }
    }
}
