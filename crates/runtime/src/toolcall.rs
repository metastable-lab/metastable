use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use async_openai::types::FunctionCall;

pub trait ExecutableFunctionCall:
    Clone + Serialize + DeserializeOwned + Send + Sync + 'static
{
    fn name() -> &'static str;
    fn from_function_call(function_call: FunctionCall) -> Result<Self>;
    #[allow(async_fn_in_trait)]
    async fn execute(&self) -> Result<String>;
}

#[macro_export]
macro_rules! define_function_types {
    (
        $(
            $variant:ident($type:ty, $name:expr)
        ),* $(,)?
    ) => {
        use serde::{Deserialize, Serialize};
        use async_openai::types::FunctionCall;

        #[derive(Debug, Serialize, Deserialize)]
        pub enum RuntimeFunctionType {
            $($variant($type)),*
        }

        impl ExecutableFunctionCall for RuntimeFunctionType {
            fn name() -> &'static str { "runtime_function_calls" }

            fn from_function_call(function_call: FunctionCall) -> Result<Self> {
                let args: serde_json::Value = serde_json::from_str(&function_call.arguments)?;
                
                match function_call.name.as_str() {
                    $(
                        $name => {
                            let function = serde_json::from_value::<$type>(args)?;
                            Ok(RuntimeFunctionType::$variant(function))
                        }
                    ),*,
                    _ => Err(anyhow::anyhow!("Unknown function type: {}", function_call.name))
                }
            }

            async fn execute(&self) -> Result<String> {
                match self {
                    $(
                        RuntimeFunctionType::$variant(f) => f.execute().await
                    ),*
                }
            }
        }

        impl Clone for RuntimeFunctionType {
            fn clone(&self) -> Self {
                match self {
                    $(
                        RuntimeFunctionType::$variant(f) => RuntimeFunctionType::$variant(f.clone())
                    ),*
                }
            }
        }
    };
}   