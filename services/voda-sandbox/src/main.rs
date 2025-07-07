use std::{sync::Arc, time::Instant};

use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use sqlx::{PgPool, types::Uuid};
use termimad::crossterm::style::{Attribute, Color};
use termimad::MadSkin;
use tokio::sync::mpsc;
// use tracing::Level;

use tracing::Level;
use voda_database::{init_db_pool, QueryCriteria, SqlxCrud, SqlxFilterQuery};
use voda_runtime::user::{UserProfile, UserUrl};
use voda_runtime::{MessageRole, RuntimeClient, SystemConfig, User, UserMetadata, UserPoints, UserUsage};
use voda_runtime_roleplay::{AuditLog, Character, RoleplayMessage, RoleplayRawMemory, RoleplayRuntimeClient, RoleplaySession};

mod config;
use config::{SYSTEM_CONFIG, TEST_CHARACTER, TEST_USER};

init_db_pool!(
    User, UserUsage, UserProfile, SystemConfig, UserPoints, UserMetadata, UserUrl,
    Character, RoleplaySession, RoleplayMessage, AuditLog
);

fn create_skin() -> MadSkin {
    let mut skin = MadSkin::default_dark();

    // Bold for speech: Bright, friendly yellow.
    skin.bold.set_fg(Color::Rgb {
        r: 255,
        g: 255,
        b: 85,
    });

    // Italic for actions: A dimmer, greyish color, and ensure it's italic.
    skin.italic.set_fg(Color::AnsiValue(245));
    skin.italic.add_attr(Attribute::Italic);

    // List items: A gold-like chevron.
    skin.bullet = termimad::StyledChar::from_fg_char(
        Color::Rgb {
            r: 255,
            g: 215,
            b: 0,
        },
        '❯',
    );

    // The rest of the text.
    skin.paragraph.set_fg(Color::AnsiValue(252));

    skin
}

async fn get_or_create_session(
    db: &PgPool,
    user: &User,
    character: &Character,
    system_config: &SystemConfig,
) -> Result<RoleplaySession> {
    if let Some(session) = RoleplaySession::find_one_by_criteria(QueryCriteria::new(), db).await? {
        Ok(session)
    } else {
        let session = RoleplaySession {
            id: Uuid::new_v4(),
            public: true,
            owner: user.id.clone(),
            character: character.id.clone(),
            system_config: system_config.id.clone(),
            history: vec![],
            updated_at: voda_common::get_current_timestamp(),
            created_at: voda_common::get_current_timestamp(),
        };
        session.create(&*db).await.map_err(anyhow::Error::from)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::DEBUG).init();
    println!("{}", "Sandbox CLI initializing ... ".red());

    let db_pool = Arc::new(connect(false).await.clone());

    let user = TEST_USER.clone().create(&*db_pool).await?;
    
    let mut character_data = TEST_CHARACTER.clone();
    character_data.creator = user.id.clone();
    let character = character_data.create(&*db_pool).await?;

    let system_config = SYSTEM_CONFIG.clone().create(&*db_pool).await?;
    let session = get_or_create_session(&*db_pool, &user, &character, &system_config).await?;

    let memory = RoleplayRawMemory::new(db_pool.clone()).await;

    let (tx, _rx) = mpsc::channel(100);
    let client =
        RoleplayRuntimeClient::new(db_pool.clone(), Arc::new(memory), tx).await;

    client.on_init().await?;

    let mut rl = DefaultEditor::new()?;
    println!("{}", "Sandbox CLI Initialized. Type 'exit' or press Ctrl-D to quit.".green());

    // Display character's first messages
    println!("{}:", "Bot:".yellow());
    let skin = create_skin();
    println!("{}", skin.term_text(&character.prompts_first_message));

    loop {
        let readline = rl.readline(&format!("{} ", "You:".green()));
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() { continue; }
                if input == "exit" { break; }

                rl.add_history_entry(input)?;

                let user_message = RoleplayMessage {
                    id: Uuid::new_v4(),
                    session_id: session.id.clone(),
                    owner: user.id.clone(),
                    role: MessageRole::User,
                    content_type: voda_runtime::MessageType::Text,
                    content: input.to_string(),
                    created_at: voda_common::get_current_timestamp(),
                    updated_at: voda_common::get_current_timestamp(),
                };

                let start_time = Instant::now();
                let pb = ProgressBar::new_spinner();
                pb.enable_steady_tick(std::time::Duration::from_millis(120));
                pb.set_style(
                    ProgressStyle::with_template("{spinner:.blue} {msg} [{elapsed_precise}]")
                        .unwrap()
                        .tick_strings(&[
                            "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
                        ]),
                );
                pb.set_message("Bot is thinking...");

                let response = client.on_new_message(&user_message).await?;

                pb.finish_and_clear();
                let elapsed = start_time.elapsed();

                println!("{}:", "Bot:".yellow());
                let skin = create_skin();
                println!("{}", skin.term_text(&response.content));

                let mut details = Vec::new();
                let usage = &response.usage;
                details.push(format!(
                    "tokens: prompt={}, completion={}, total={}",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                ));

                let calls = &response.maybe_function_call;
                if !calls.is_empty() {
                    let call_names: Vec<String> =
                        calls.iter().map(|c| c.name.clone()).collect();
                    details.push(format!("function_calls: [{}]", call_names.join(", ")));
                }

                println!(
                    "{}",
                    format!(
                        "[Response in {:.2}s | {}]",
                        elapsed.as_secs_f32(),
                        details.join(" | ")
                    )
                    .dimmed()
                );
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "Interrupted. Exiting.".red());
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "Exiting.".red());
                break;
            }
            Err(err) => {
                println!("{} {:?}", "Error:".red(), err);
                break;
            }
        }
    }

    Ok(())
} 