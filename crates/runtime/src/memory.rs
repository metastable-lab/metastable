use anyhow::Result;

use sqlx::types::Uuid;
use crate::{Message, SystemConfig};

#[async_trait::async_trait]
pub trait Memory: Clone + Send + Sync + 'static {
    type MessageType: Message;

    async fn initialize(&mut self) -> Result<()>;

    async fn add(&self, messages: &[Self::MessageType]) -> Result<()>;
    async fn search(&self, message: &Self::MessageType, limit: u64) -> Result<
        (Vec<Self::MessageType>, SystemConfig)
    >;

    async fn update(&self, messages: &[Self::MessageType]) -> Result<()>;
    async fn delete(&self, message_ids: &[Uuid]) -> Result<()>;
    async fn reset(&self, user_id: &Uuid) -> Result<()>;
}

#[macro_export]
macro_rules! define_composed_memory {
    (
        $struct_name:ident,
        $message_enum_name:ident,
        $($field_name:ident: ($mem_ty:ty, $msg_ty:ty)),+
    ) => {
        #[derive(Clone)]
        pub struct $struct_name {
            $(pub $field_name: $mem_ty),+
        }

        #[derive(Clone, Debug)]
        pub enum $message_enum_name {
            $($field_name($msg_ty)),+
        }

        $(
            impl From<$msg_ty> for $message_enum_name {
                fn from(m: $msg_ty) -> Self {
                    $message_enum_name::$field_name(m)
                }
            }
        )+

        impl $crate::Message for $message_enum_name {
            fn id(&self) -> &Uuid {
                match self {
                    $($message_enum_name::$field_name(m) => m.id()),+
                }
            }

            fn role(&self) -> &$crate::message::MessageRole {
                match self {
                    $($message_enum_name::$field_name(m) => m.role()),+
                }
            }

            fn owner(&self) -> &Uuid {
                match self {
                    $($message_enum_name::$field_name(m) => m.owner()),+
                }
            }

            fn content_type(&self) -> &$crate::message::MessageType {
                match self {
                    $($message_enum_name::$field_name(m) => m.content_type()),+
                }
            }

            fn content(&self) -> Option<String> {
                match self {
                    $($message_enum_name::$field_name(m) => m.content()),+
                }
            }
        }

        #[async_trait::async_trait]
        impl $crate::memory::Memory for $struct_name {
            type MessageType = $message_enum_name;

            async fn initialize(&mut self) -> anyhow::Result<()> {
                $(self.$field_name.initialize().await?;)+
                Ok(())
            }

            async fn add(&self, messages: &[Self::MessageType]) -> anyhow::Result<()> {
                $(
                    let mut mem_messages = Vec::new();
                    for msg in messages {
                        if let $message_enum_name::$field_name(m) = msg {
                            mem_messages.push(m.clone());
                        }
                    }
                    if !mem_messages.is_empty() {
                        self.$field_name.add(&mem_messages).await?;
                    }
                )+
                Ok(())
            }

            async fn search(
                &self,
                message: &Self::MessageType,
                limit: u64
            ) -> anyhow::Result<(Vec<Self::MessageType>, $crate::SystemConfig)> {
                let mut all_results = Vec::new();
                let mut final_config = $crate::SystemConfig::default();

                $(
                    let temp_message = <$msg_ty>::from_message(message);
                    let (results, config) = self.$field_name.search(&temp_message, limit).await?;
                    all_results.extend(results.into_iter().map($message_enum_name::from));
                    final_config.merge(config);
                )+

                all_results.sort_by_key(|m| m.created_at());
                all_results.dedup_by_key(|m| *m.id());
                let limited_results = all_results.into_iter().take(limit as usize).collect();

                Ok((limited_results, final_config))
            }

            async fn update(&self, messages: &[Self::MessageType]) -> anyhow::Result<()> {
                $(
                    let mut mem_messages = Vec::new();
                    for msg in messages {
                        if let $message_enum_name::$field_name(m) = msg {
                            mem_messages.push(m.clone());
                        }
                    }
                    if !mem_messages.is_empty() {
                        self.$field_name.update(&mem_messages).await?;
                    }
                )+
                Ok(())
            }

            async fn delete(&self, message_ids: &[Uuid]) -> anyhow::Result<()> {
                $(self.$field_name.delete(message_ids).await?;)+
                Ok(())
            }

            async fn reset(&self, user_id: &Uuid) -> anyhow::Result<()> {
                $(self.$field_name.reset(user_id).await?;)+
                Ok(())
            }
        }
    };
}
