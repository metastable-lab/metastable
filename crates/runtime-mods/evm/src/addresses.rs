use alloy_core::primitives::Address;
use alloy_core::primitives::address;

pub const MULTICALL_ADDRESS: Address = address!("cA11bde05977b3631167028862bE2a173976CA11");

pub mod sei {
    use super::*;
    pub const WSEI: Address = address!("E30feDd158A2e3b13e9badaeABaFc5516e95e8C7");
    pub const TAKARA_LEND: Address = address!("A26b9BFe606d29F16B5Aecf30F9233934452c4E2");
    pub const GITCOIN_ADDRESS: Address = address!("1E18cdce56B3754c4Dca34CB3a7439C24E8363de");
}

pub mod avax {
    use super::*;
    pub const WAVAX: Address = address!("B31f66AA3C1e785363F0875A1B74E27b85FD66c7");
    pub const P_ROUTER: Address = address!("AAA45c8F5ef92a000a121d102F4e89278a711Faa");
}