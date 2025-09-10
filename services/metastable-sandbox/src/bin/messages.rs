use std::sync::Arc;

use anyhow::Result;
use async_openai::types::FunctionCall;
use sqlx::types::Json;
use metastable_database::SqlxCrud;
use metastable_database::{init_databases, QueryCriteria, SqlxFilterQuery};
use metastable_runtime_roleplay::agents::SummarizeCharacter;
use metastable_runtime_roleplay::agents::SendMessage;

use metastable_runtime::{Message, ToolCall};

init_databases!(
    default: [
        metastable_runtime::User,
        metastable_runtime::UserUrl,
        metastable_runtime::UserReferral,
        metastable_runtime::UserBadge,
        metastable_runtime::UserFollow,

        metastable_runtime::SystemConfig,

        metastable_runtime::CardPool,
        metastable_runtime::Card,
        metastable_runtime::DrawHistory,

        metastable_runtime::Message,
        metastable_runtime::ChatSession,
        metastable_runtime::UserPointsLog,

        metastable_runtime::Character,
        metastable_runtime::CharacterHistory,
        metastable_runtime::CharacterSub,
        metastable_runtime::AuditLog,
    ],
    pgvector: [ 
        metastable_clients::EmbeddingMessage
    ]
);

pub fn validate_parsing(m: &SendMessage) -> Result<FunctionCall> {
    let tc = m.into_tool_call()?;
    let mm = SendMessage::try_from_tool_call(&tc)?;
    if *m != mm {
        return Err(anyhow::anyhow!("Parsing failed"));
    }
    Ok(tc)
}

pub fn try_prase_message(message: &Message) -> Result<FunctionCall> {
    let t = if let Some(tc) = &message.assistant_message_tool_call.0 {
        let function_name = &tc.name;
        if function_name == "summarize_character" {
            let t = SummarizeCharacter::try_from_tool_call(&tc)?;
            t.into_tool_call()?
        } else { // Assumes send_message
            let t = SendMessage::try_from_tool_call(&tc)?;
            let parsed_tool = SendMessage::from_legacy_inputs(&message.assistant_message_content, &t);
            validate_parsing(&parsed_tool)?
        }
    }  else {
        // No tool call, but has content.
        let assistant_content = message.assistant_message_content.trim();
        let cleaned_content = assistant_content.trim_matches(|c| c == '*' || c == '.').trim();

        let parsed_tool = SendMessage::from_legacy_inputs(cleaned_content, &SendMessage::default());
        validate_parsing(&parsed_tool)?
    };

    Ok(t)
}


#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    
    let run_migrations = false;
    let db = Arc::new(connect(false, false, run_migrations).await.clone());
    let _pgvector_db = Arc::new(connect_pgvector(false, false, run_migrations).await.clone());


    let mut tx = db.begin().await?;
    Message::toggle_trigger(&mut *tx, false).await?;
    let messages = Message::find_by_criteria(
        QueryCriteria::new()
            .add_valued_filter("is_migrated", "=", false),
        &mut *tx
    ).await?;
    tx.commit().await?;

    for mm in messages.chunks(100) {
        let mut tx = db.begin().await?;
        for m in mm {
            let mut m = m.clone();
            let t = try_prase_message(&m)?;
            println!("{:?}", t);
            m.assistant_message_tool_call = Json(Some(t));
            m.is_migrated = true;
            m.update(&mut *tx).await?;
        }
        tx.commit().await?;
        println!("{} messages processed", mm.len());
    }

    let mut tx = db.begin().await?;
    Message::toggle_trigger(&mut *tx, true).await?;
    tx.commit().await?;

    println!("Database initialized successfully");
    Ok(())
}