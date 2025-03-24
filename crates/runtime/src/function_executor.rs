use std::marker::PhantomData;

use anyhow::{anyhow, Result};
use async_openai::types::FunctionCall;
use tokio::sync::{mpsc, oneshot};

use crate::ExecutableFunctionCall;

pub struct FunctionExecutor<F: ExecutableFunctionCall> {
    execution_queue: mpsc::Receiver<(FunctionCall, oneshot::Sender<Result<String>>)>,
    _phantom: PhantomData<F>,
}

impl<F: ExecutableFunctionCall> FunctionExecutor<F> {
    pub fn new(
        execution_queue: mpsc::Receiver<(FunctionCall, oneshot::Sender<Result<String>>)>
    ) -> Self {
        Self { execution_queue, _phantom: PhantomData }
    }

    pub async fn run(&mut self) {
        while let Some((call, tx)) = self.execution_queue.recv().await {
            if let Ok(f) = F::from_function_call(call.clone()) {
                let result = f.execute().await;
                tx.send(result).expect("message channel closed");
            } else {
                tracing::error!("Invalid function call: {:?}", call);
                tx.send(Err(anyhow!("Invalid function call"))).expect("message channel closed");
            }
        }
    }
}
