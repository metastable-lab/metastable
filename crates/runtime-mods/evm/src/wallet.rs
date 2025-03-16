use alloy_core::primitives::{keccak256, Address};
use alloy_network::EthereumWallet;
use alloy_signer_local::PrivateKeySigner;

pub struct LocalWallet {
    private_key: [u8; 32],
    wallet: PrivateKeySigner,
}

impl LocalWallet {
    pub fn _new(salt: &[u8; 32]) -> Self {
        let mut s = 0;
        let mut try_private_key = keccak256(&[salt.to_vec(), salt.to_vec()].concat());
        let wallet = loop {
            if let Ok(wallet) = PrivateKeySigner::from_slice(&try_private_key.to_vec()) {
                break wallet;
            } else {
                try_private_key = keccak256(&[try_private_key.to_vec(), [s].to_vec()].concat());
                s += 1;
            }
        };

        Self { 
            private_key: try_private_key.try_into().unwrap(), 
            wallet 
        }
    }

    pub fn from_private_key(private_key: &[u8; 32]) -> Self {
        Self {
            private_key: private_key.clone(), 
            wallet: PrivateKeySigner::from_slice(private_key).unwrap() 
        }
    }

    pub fn into_alloy_wallet(&self) -> EthereumWallet {
        EthereumWallet::from(self.wallet.clone())
    }

    pub fn eth_address(&self) -> Address {
        self.wallet.address()
    }

    pub fn private_key(&self) -> &[u8; 32] {
        &self.private_key
    }
}
