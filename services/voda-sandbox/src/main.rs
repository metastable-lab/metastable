use std::{sync::Arc, time::Instant};

use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use termimad::crossterm::style::{Attribute, Color};
use termimad::MadSkin;
use tokio::sync::mpsc;
// use tracing::Level;

use sqlx::PgPool;
use voda_common::CryptoHash;
use voda_database::{init_db_pool, QueryCriteria, SqlxCrud, SqlxFilterQuery, SqlxPopulateId};
use voda_runtime::{MessageRole, RuntimeClient, User};
use voda_runtime_roleplay::{Character, RoleplayMessage, RoleplayRuntimeClient, RoleplaySession};

mod config;
use config::{SYSTEM_CONFIG, TEST_CHARACTER, TEST_USER};

init_db_pool!(User, Character, RoleplaySession, RoleplayMessage);

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
) -> Result<RoleplaySession> {
    if let Some(session) = RoleplaySession::find_one_by_criteria(QueryCriteria::new(), db).await? {
        Ok(session)
    } else {
        let mut session = RoleplaySession {
            id: CryptoHash::default(),
            public: true,
            owner: user.id.clone(),
            character_id: character.id.clone(),
            history: vec![],
            updated_at: voda_common::get_current_timestamp(),
            created_at: voda_common::get_current_timestamp(),
        };
        session.sql_populate_id()?;
        session.create(&*db).await.map_err(anyhow::Error::from)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    println!("{}", "Sandbox CLI initializing ... ".red());

    let db_pool = Arc::new(connect().await.clone());

    let user = TEST_USER.clone().create(&*db_pool).await?;
    let character = TEST_CHARACTER.clone().create(&*db_pool).await?;
    let mut session = get_or_create_session(&*db_pool, &user, &character).await?;

    let (tx, _rx) = mpsc::channel(100);
    let client =
        RoleplayRuntimeClient::new(db_pool.clone(), SYSTEM_CONFIG.clone(), tx).await;

    let mut rl = DefaultEditor::new()?;
    println!("{}", "Sandbox CLI Initialized. Type 'exit' or press Ctrl-D to quit.".green());

    // Display character's first message
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

                let mut user_message = RoleplayMessage {
                    id: CryptoHash::random(),
                    session_id: session.id.clone(),
                    owner: user.id.clone(),
                    character_id: character.id.clone(),
                    role: MessageRole::User,
                    content_type: voda_runtime::MessageType::Text,
                    content: input.to_string(),
                    created_at: voda_common::get_current_timestamp(),
                };

                user_message.sql_populate_id()?;
                let user_message = user_message.create(&*db_pool).await?;

                session
                    .append_message_to_history(
                        &user_message.id,
                        voda_common::get_current_timestamp(),
                        &*db_pool,
                    )
                    .await?;

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

                let mut assistant_message = RoleplayMessage {
                    id: CryptoHash::default(),
                    session_id: session.id.clone(),
                    owner: user.id.clone(), // Or character creator ID
                    character_id: character.id.clone(),
                    role: MessageRole::Assistant,
                    content_type: voda_runtime::MessageType::Text,
                    content: response.content,
                    created_at: voda_common::get_current_timestamp(),
                };
                assistant_message.sql_populate_id()?;
                let assistant_message = assistant_message.create(&*db_pool).await?;

                session
                    .append_message_to_history(
                        &assistant_message.id,
                        voda_common::get_current_timestamp(),
                        &*db_pool,
                    )
                    .await?;
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