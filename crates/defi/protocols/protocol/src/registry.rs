use alloy::{
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use anyhow::Context;
use hashbrown::HashMap;

use crate::{DiscoverableProtocol, ProtocolFactory, ProtocolIdentifier};

pub struct ProtocolRegistry<P> {
    chain_id: ChainId,
    provider: P,
    protocols: HashMap<ProtocolIdentifier, Box<dyn DiscoverableProtocol<P>>>,
}

impl<P: Provider> ProtocolRegistry<P> {
    pub async fn new(provider: P) -> anyhow::Result<Self> {
        Ok(Self {
            chain_id: provider
                .get_chain_id()
                .await
                .context("failed to get chain id")?,
            protocols: HashMap::default(),
            provider,
        })
    }

    pub fn with<F: ProtocolFactory<P> + 'static>(mut self) -> anyhow::Result<Self> {
        self.protocols.insert(
            F::IDENTIFIER.into(),
            Box::new(
                F::new(self.chain_id)
                    .context(format!("failed to create protocol `{}`", F::IDENTIFIER))?,
            ),
        );

        Ok(self)
    }

    pub async fn discover(
        &self,
        discovered_blocks: &HashMap<ProtocolIdentifier, BlockNumber>,
        to: BlockNumber,
    ) -> anyhow::Result<HashMap<ProtocolIdentifier, Vec<Address>>>
    where
        P: Clone,
    {
        let mut discovered = HashMap::new();

        for (identifier, protocol) in &self.protocols {
            let discovered_block = *discovered_blocks.get(identifier).unwrap_or(&0);

            if discovered_block > to {
                continue;
            }

            discovered.insert(
                identifier.to_owned(),
                protocol
                    .discover(discovered_block, to, self.provider.clone())
                    .await
                    .context(format!("failed to discover protocol `{}`", identifier))?,
            );
        }

        Ok(discovered)
    }

    pub fn protocol_identifiers(&self) -> Vec<ProtocolIdentifier> {
        self.protocols.keys().cloned().collect()
    }
}
