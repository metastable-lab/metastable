
#[async_trait::async_trait]
pub trait AgentRouter {
    type Input;
    type Output;
    async fn route(&self, caller: &sqlx::types::Uuid, input: Self::Input) -> anyhow::Result<Self::Output>;
}

#[macro_export]
macro_rules! define_agent_router {
    (
        $( $variant:ident as $field:ident ($agent_type:ty) ),* $(,)?
    ) => {
        #[derive(Clone)]
        pub struct AgentsRouter {
            $( pub $field: $agent_type ),*
        }

        #[derive(Debug, Clone)]
        pub enum AgentRouterInput {
            $( $variant(<
                $agent_type as ::metastable_runtime::Agent
            >::Input) ),*
        }

        #[derive(Debug)]
        pub enum AgentRouterOutput {
            $( $variant(::metastable_runtime::Message, <
                $agent_type as ::metastable_runtime::Agent
            >::Tool, Option<serde_json::Value>) ),*
        }

        impl AgentsRouter {
            pub async fn new() -> anyhow::Result<Self> {
                Ok(Self {
                    $( $field: <$agent_type>::new().await? ),*
                })
            }
        }

        #[async_trait::async_trait]
        impl AgentRouter for AgentsRouter {
            type Input = AgentRouterInput;
            type Output = AgentRouterOutput;

            async fn route(&self, caller: &sqlx::types::Uuid, input: Self::Input) -> anyhow::Result<Self::Output> {
                match input {
                    $(
                        AgentRouterInput::$variant(input) => {
                            let (message, tool, value) = <$agent_type as ::metastable_runtime::Agent>::call(&self.$field, caller, &input).await?;
                            Ok(AgentRouterOutput::$variant(message, tool, value))
                        }
                    ),*
                }
            }
        }
    };
}

// use metastable_roleplay::{
//     agents::{RoleplayV0Agent, RoleplayV1Agent, SendMessage},
//     input::RoleplayInput,
// };

// define_agent_router! {
//     RoleplayV0 as roleplay_v0 (RoleplayV0Agent),
//     RoleplayV1 as roleplay_v1 (RoleplayV1Agent),
// }
