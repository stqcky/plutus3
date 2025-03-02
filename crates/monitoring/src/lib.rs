pub mod health;

use std::time::Instant;

use alloy::{
    primitives::{U256, map::AddressMap},
    providers::{Provider, ext::DebugApi},
    rpc::types::{
        Header,
        trace::geth::{
            GethDebugBuiltInTracerType, GethDebugTracerType, GethDebugTracingOptions,
            GethDefaultTracingOptions, PreStateConfig, PreStateFrame, TraceResult,
        },
    },
};
use futures::StreamExt as _;
use hashbrown::HashMap;
use lazy_static::lazy_static;
use tokio::sync::mpsc::Sender;

lazy_static! {
    static ref TRACING_OPTIONS: GethDebugTracingOptions = GethDebugTracingOptions {
        config: GethDefaultTracingOptions {
            enable_memory: None,
            disable_memory: Some(true),
            disable_stack: Some(true),
            disable_storage: Some(false),
            enable_return_data: None,
            disable_return_data: Some(true),
            debug: None,
            limit: None,
        },
        tracer: Some(GethDebugTracerType::BuiltInTracer(
            GethDebugBuiltInTracerType::PreStateTracer,
        )),
        tracer_config: PreStateConfig {
            diff_mode: Some(true),
            disable_code: Some(true),
            disable_storage: Some(true),
        }
        .into(),
        timeout: None,
    };
}

pub struct StateChange {
    pub block_header: Header,
    pub changes: Vec<AddressMap<HashMap<U256, U256>>>,
}

#[derive(Clone)]
pub struct StateMonitor<P> {
    provider: P,
}

impl<P: Provider + Clone + 'static> StateMonitor<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub async fn subscribe_blocks(&self, tx: Sender<StateChange>) -> anyhow::Result<()> {
        let monitor = self.clone();

        tokio::spawn({
            let mut blocks = monitor.provider.subscribe_blocks().await?.into_stream();

            async move {
                while let Some(header) = blocks.next().await {
                    let state = monitor.get_state_changes(header).await;

                    tx.send(state).await.expect("channel is open");
                }
            }
        });

        Ok(())
    }

    pub async fn get_state_changes(&self, block_header: Header) -> StateChange {
        let now = Instant::now();

        let changes = self
            .provider
            .debug_trace_block_by_number(block_header.number.into(), TRACING_OPTIONS.clone())
            .await
            .unwrap()
            .into_iter()
            .filter_map(|trace| match trace {
                TraceResult::Success { result, .. } => result
                    .try_into_pre_state_frame()
                    .map(|frame| match frame {
                        PreStateFrame::Diff(diff) => diff.post,
                        _ => unreachable!(),
                    })
                    .ok(),
                _ => None,
            })
            .collect::<Vec<_>>();

        let changes = changes
            .into_iter()
            .map(|changes| {
                AddressMap::from_iter(changes.into_iter().map(|(address, state)| {
                    (
                        address,
                        HashMap::from_iter(
                            state
                                .storage
                                .into_iter()
                                .map(|(slot, value)| (slot.into(), value.into())),
                        ),
                    )
                }))
            })
            .collect();

        // tracing::info!("get_state_changes: {:?}", now.elapsed());

        StateChange {
            block_header,
            changes,
        }
    }
}
