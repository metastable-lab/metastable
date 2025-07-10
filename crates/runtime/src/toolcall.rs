use anyhow::Result;
use async_openai::types::FunctionCall;
use serde::{de::DeserializeOwned, Serialize};

use crate::LLMRunResponse;

#[async_trait::async_trait]
pub trait ExecutableFunctionCall:
    Clone + Serialize + DeserializeOwned + Send + Sync + 'static
{
    type CTX: Send + Sync + 'static;
    type RETURN: Send + Sync + 'static + std::fmt::Debug + std::clone::Clone;

    fn name() -> &'static str;
    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        Ok(serde_json::from_str(&function_call.arguments)?)
    }

    async fn execute(&self, 
        llm_response: &LLMRunResponse, 
        execution_context: &Self::CTX
    ) -> Result<Self::RETURN>;
}

#[macro_export]
macro_rules! toolcalls {
    (
        ctx: $ctx:ty,
        tools: [
            $(
                ($type:ident, $name:expr, $return_type:ty)
            ),* $(,)?
        ]
    ) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub enum RuntimeToolcall {
            $(
                $type($type),
            )*
        }

        #[derive(Debug, Clone)]
        pub enum RuntimeToolcallReturn {
            $(
                $type($return_type),
            )*
        }

        #[async_trait::async_trait]
        impl ::voda_runtime::ExecutableFunctionCall for RuntimeToolcall {
            type CTX = $ctx;
            type RETURN = RuntimeToolcallReturn;

            fn name() -> &'static str { "runtime_function_calls" }

            fn from_function_call(function_call: async_openai::types::FunctionCall) -> anyhow::Result<Self> {
                let args: serde_json::Value = serde_json::from_str(&function_call.arguments)?;

                match function_call.name.as_str() {
                    $(
                        $name => {
                            let function = serde_json::from_value::<$type>(args)?;
                            Ok(RuntimeToolcall::$type(function))
                        }
                    ),*
                    _ => Err(anyhow::anyhow!("Unknown function type: {}", function_call.name))
                }
            }

            async fn execute(&self, 
                llm_response: &::voda_runtime::LLMRunResponse, 
                execution_context: &Self::CTX
            ) -> anyhow::Result<Self::RETURN> {
                match self {
                    $(
                        RuntimeToolcall::$type(f) => {
                            let result = f.execute(llm_response, execution_context).await?;
                            Ok(RuntimeToolcallReturn::$type(result))
                        }
                    ),*
                }
            }
        }
    };
}   