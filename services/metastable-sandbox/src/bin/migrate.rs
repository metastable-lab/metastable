use std::sync::Arc;

use anyhow::Result;
use metastable_database::{init_databases, QueryCriteria, SqlxFilterQuery};
use metastable_runtime_roleplay::agents::SummarizeCharacter;
use metastable_runtime_roleplay::agents::SendMessage;

use metastable_runtime::{Message, ToolCall};
use metastable_sandbox::legacy::{
    RoleplayMessage, 
    RoleplaySession as LegacyRoleplaySession, 
    CharacterCreationMessage as LegacyCharacterCreationMessage
};

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
    

        LegacyCharacterCreationMessage,
        LegacyRoleplaySession,
        RoleplayMessage,
    ],
    pgvector: [ 
        metastable_clients::EmbeddingMessage
    ]
);

// async fn migrate_messages(db: &Arc<PgPool>) -> Result<()> {
//     let mut tx = db.begin().await?;
//     LegacyRoleplaySession::toggle_trigger(&mut *tx, false).await?;
//     Message::toggle_trigger(&mut *tx, false).await?;
//     ChatSession::toggle_trigger(&mut *tx, false).await?;

//     let sessions = LegacyRoleplaySession::find_by_criteria(
//         QueryCriteria::new()
//             .add_valued_filter("is_migrated", "=", false)
//             ,
//         &mut *tx
//     ).await?;
//     tx.commit().await?;

//     tracing::info!("Found {} sessions", sessions.len());
//     let mut count = 0;
//     for session in sessions {
//         let mut tx = db.begin().await?;

//         let mut s = session.clone();
        
//         tracing::info!("Migrating session {}", session.id);
//         let new_session = ChatSession {
//             id: session.id,
//             public: false,
//             owner: session.owner,
//             character: session.character,
//             use_character_memory: session.use_character_memory,
//             hidden: session.hidden,
//             nonce: 0,
//             user_mask: None,
//             updated_at: session.updated_at,
//             created_at: session.created_at,
//         };
//         let new_session = new_session.create(&mut *tx).await?;
//         new_session.clone().force_set_timestamp(&mut *tx, session.created_at, session.updated_at).await?;

//         tracing::info!("Fetching messages for session {}", count);
//         count += 1;
//         let mut messages = session.fetch_history(&mut *tx).await?;

//         // Order messages by created_at
//         messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));

//         // Separate user and assistant messages
//         let mut user_messages = Vec::new();
//         let mut assistant_messages = Vec::new();

//         for msg in &messages {
//             let mut updated_msg = msg.clone();
//             updated_msg.is_migrated = true;
//             updated_msg.update(&mut *tx).await?;

//             match msg.role {
//                 MessageRole::User => user_messages.push(msg),
//                 MessageRole::Assistant => assistant_messages.push(msg),
//                 _ => {}
//             }
//         }

//         // Pair user and assistant messages by order
//         let pairs = user_messages.iter().zip(assistant_messages.iter()).map(|(u, a)| (*u, *a)).collect::<Vec<_>>();

//         // Now `pairs` contains tuples of (user_message, assistant_message) in order
//         // You can process these pairs as needed
//         let system_config = session.fetch_system_config(&mut *tx).await?
//             .ok_or(anyhow::anyhow!("No system config found for session {}", session.id))?;

//         for (user_msg, assistant_msg) in pairs {
//             let mut message = RoleplayMessage::to_message(&system_config, &user_msg, &assistant_msg);
//             message.session = Some(new_session.id);
//             let msg = message.create(&mut *tx).await?;
//             msg.force_set_timestamp(&mut *tx, user_msg.created_at, assistant_msg.created_at).await?;
//         }
//         tracing::info!("session {:?} migrated to {:?}", session.id, new_session.id);

//         s.is_migrated = true;
//         s.update(&mut *tx).await?;
//         tx.commit().await?;
//     }

//     let mut tx = db.begin().await?;
//     LegacyRoleplaySession::toggle_trigger(&mut *tx, false).await?;
//     Message::toggle_trigger(&mut *tx, false).await?;
//     ChatSession::toggle_trigger(&mut *tx, false).await?;
//     tx.commit().await?;
//     Ok(())
// }

// async fn migrate_characters(db: &Arc<PgPool>) -> Result<()> {
//     tracing::info!("Migrating characters");
//     let mut tx = db.begin().await?;
//     Character::toggle_trigger(&mut *tx, false).await?;

//     let character_creation_messages = LegacyCharacterCreationMessage::find_by_criteria(
//         QueryCriteria::new()
//             .add_valued_filter("is_migrated", "=", false),
//         &mut *tx
//     ).await?;

//     for mut message in character_creation_messages {
//         message.is_migrated = true;
//         let character = message.fetch_character_creation_maybe_character_id(&mut *tx).await?;
//         if character.is_none() {
//             tracing::warn!("Character not found for message {}", message.id);
//         } else {
//             let char = character.unwrap();
//             let updated_character = message.update_character(&char)?;

//             tracing::info!("Migrating character {}", updated_character.id);
//             updated_character.update(&mut *tx).await?;
//         }
       
//         message.update(&mut *tx).await?;
//     }

//     tx.commit().await?;
//     Ok(())
// }
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
    let messages = Message::find_by_criteria(
        QueryCriteria::new(),
        &mut *tx
    ).await?;

    for message in messages {
        if let Some(tc) = message.assistant_message_tool_call.0 {
            let function_name = &tc.name;

            if function_name == "summarize_character" {
                if let Err(e) = SummarizeCharacter::try_from_tool_call(&tc) {
                    println!("--- FAILED PARSE (SummarizeCharacter) | Message ID: {} ---", message.id);
                    println!("Error: {}", e);
                    println!("Original Toolcall: {:?}", tc);
                    println!("----------------------------------------------------------");
                }
            } else { // Assumes send_message
                let send_message_tool = match SendMessage::try_from_tool_call(&tc) {
                    Ok(tool) => tool,
                    Err(e) => {
                        println!("--- FAILED PARSE (SendMessage initial) | Message ID: {} ---", message.id);
                        println!("Error: {}", e);
                        println!("Original Toolcall: {:?}", tc);
                        println!("------------------------------------------------------------");
                        continue;
                    }
                };

                let parsed_tool = SendMessage::from_legacy_inputs(&message.assistant_message_content, &send_message_tool);
                if let Err(e) = parsed_tool.into_tool_call() {
                    println!("--- FAILED PARSE (SendMessage legacy) | Message ID: {} ---", message.id);
                    println!("Error: {}", e);
                    println!("Content: {}", message.assistant_message_content);
                    println!("Original Toolcall: {:?}", tc);
                    println!("---------------------------------------------------------");
                } else {
                    let new_tool_call = parsed_tool.into_tool_call().unwrap();
                    let parsed_back = SendMessage::try_from_tool_call(&new_tool_call).unwrap();
                    if parsed_tool != parsed_back {
                        println!("--- FAILED PARSE (SendMessage legacy - parse back) | Message ID: {} ---", message.id);
                        println!("Parsed tool: {:?}", parsed_tool);
                        println!("Parsed back: {:?}", parsed_back);
                        println!("---------------------------------------------------------");
                    }
                    println!("--- PASSED PARSE (SendMessage legacy) | Message ID: {} ---", message.id);
                    println!("Toolcall: {:?}", new_tool_call);
                    println!("---------------------------------------------------------");
                }
            }
        } else {
            // No tool call, but has content.
            let assistant_content = message.assistant_message_content.trim();
            let cleaned_content = assistant_content.trim_matches(|c| c == '*' || c == '.').trim();
            let is_done_marker = cleaned_content == "内容生成完毕";

            if !assistant_content.is_empty() && !is_done_marker {
                let parsed_tool = SendMessage::from_legacy_inputs(assistant_content, &SendMessage::default());
                println!("{:?}", parsed_tool);                
                let is_parsed_tool_empty = parsed_tool.messages.is_empty() && parsed_tool.options.is_empty() && parsed_tool.summary.is_empty();

                if !is_parsed_tool_empty {
                    if let Err(e) = parsed_tool.into_tool_call() {
                        println!("--- FAILED PARSE (Legacy Content Only) | Message ID: {} ---", message.id);
                        println!("Error: {}", e);
                        println!("Content: {}", assistant_content);
                        println!("------------------------------------------------------------");
                    } else {
                        println!("--- PASSED PARSE (Legacy Content Only) | Message ID: {} ---", message.id);
                        println!("Toolcall: {:?}", parsed_tool.into_tool_call().unwrap());
                        println!("---------------------------------------------------------");
                    }
                }
            }
        }
    }

    tx.rollback().await?;


    // migrate_messages(&db).await?;
    // migrate_characters(&db).await?;

    // let mut tx = db.begin().await?;
    // // // start dumping shit into the db
    // // let admin = get_admin_user();
    // // admin.create(&mut *tx).await?;

    // let users = get_admin_users();
    // for user in users {
    //     user.create(&mut *tx).await?;
    // }
    // tx.commit().await?;

    // let normal_user = get_normal_user();
    // normal_user.create(&mut *tx).await?;
    // tx.commit().await?;
    
    println!("Database initialized successfully");
    Ok(())
}