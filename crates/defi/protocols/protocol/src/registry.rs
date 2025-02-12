use alloy::{
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use anyhow::Context;
use hashbrown::HashMap;
use plutus_storage::Storage;

use crate::{DiscoverableProtocol, ProtocolFactory};

pub struct ProtocolRegistry<P> {
    chain_id: ChainId,
    provider: P,
    protocols: HashMap<String, Box<dyn DiscoverableProtocol<P>>>,
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
        discovered_blocks: &HashMap<String, BlockNumber>,
        to: BlockNumber,
    ) -> anyhow::Result<HashMap<String, Vec<Address>>>
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

    pub async fn discover_and_store(&self, to: BlockNumber, storage: &Storage) -> anyhow::Result<()>
    where
        P: Clone,
    {
        let last_discovered_blocks = storage
            .get_last_discovered_blocks(&self.protocol_identifiers())
            .await?;

        let discovered = self.discover(&last_discovered_blocks, to).await?;

        storage.insert_pools(&discovered).await?;

        for (protocol, _) in discovered {
            storage.set_last_discovered_block(to, &protocol).await?;
        }

        Ok(())
    }

    pub fn protocol_identifiers(&self) -> Vec<String> {
        self.protocols.keys().cloned().collect()
    }
}
