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
use async_stream::stream;
use futures::{Stream, StreamExt as _};
use hashbrown::HashMap;
use lazy_static::lazy_static;

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
    pub block: Header,
    pub changes: Vec<AddressMap<HashMap<U256, U256>>>,
}

pub struct StateMonitor;

impl StateMonitor {
    pub async fn monitor_blocks<P: Provider>(
        provider: P,
    ) -> anyhow::Result<impl Stream<Item = StateChange>> {
        let mut blocks = provider.subscribe_blocks().await?.into_stream();

        Ok(stream! {
            while let Some(block) = blocks.next().await {
                let changes = provider
                    .debug_trace_block_by_number(block.number.into(), TRACING_OPTIONS.clone())
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

                let state = StateChange { block, changes };

                yield state;
            }
        })
    }
}
