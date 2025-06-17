use std::time::Duration;

use anyhow::Result;
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{header, Client};
use rustyline::{error::ReadlineError, DefaultEditor};
use termimad::{crossterm::style::Attribute, MadSkin};
use termimad::crossterm::style::Color;
use tokio::time::sleep;
use uuid::Uuid;

use voda_database::init_db_pool;
use voda_runtime::User;

use voda_sandbox::{
    api_client::ApiClient,
    config::get_normal_user,
    graphql::{GraphQlClient, Session},
};

init_db_pool!(
    voda_runtime::User,
    voda_runtime::UserUsage,
    voda_runtime::UserUrl,
    voda_runtime::UserReferral,
    voda_runtime::UserBadge,
    voda_runtime::SystemConfig,
    voda_runtime_roleplay::Character,
    voda_runtime_roleplay::RoleplaySession,
    voda_runtime_roleplay::RoleplayMessage,
    voda_runtime_roleplay::AuditLog
);

const BASE_URL: &str = "http://localhost:3033";

/// Application state holding shared resources.
struct AppState {
    api_client: ApiClient,
    graphql_client: GraphQlClient,
    user: User,
}

/// Main application controller.
struct App {
    state: AppState,
    rl: DefaultEditor,
    skin: MadSkin,
}

impl App {
    /// Creates a new App instance, initializing all required components.
    fn new() -> Result<Self> {
        let user = get_normal_user();
        println!("Welcome, {}!", user.user_aka.cyan());
        let secret_key = std::env::var("SECRET_SALT").expect("SECRET_SALT must be set");

        let mut headers = header::HeaderMap::new();
        let token = user.generate_auth_token(&secret_key);
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", token).parse().unwrap(),
        );

        let http_client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build reqwest client");

        let api_client = ApiClient::new(BASE_URL.to_string(), http_client.clone());
        let graphql_client = GraphQlClient::new(BASE_URL.to_string(), http_client.clone());

        let state = AppState {
            api_client,
            graphql_client,
            user,
        };

        Ok(Self {
            state,
            rl: DefaultEditor::new()?,
            skin: create_skin(),
        })
    }

    /// The main application loop.
    async fn run(&mut self) -> Result<()> {
        loop {
            let session = self.select_or_create_session().await?;
            self.chat_loop(session).await?;
            println!("\n{}\n", "Returning to session selection...".bold());
        }
    }

    /// Guides the user through selecting an existing session or creating a new one.
    async fn select_or_create_session(&self) -> Result<Session> {
        match self.select_session().await? {
            Some(session) => Ok(session),
            None => self.create_new_session().await,
        }
    }

    /// Fetches sessions and prompts the user to select one.
    async fn select_session(&self) -> Result<Option<Session>> {
        println!("Fetching sessions...");
        let sessions = self
            .state
            .graphql_client
            .get_my_sessions_and_messages(&self.state.user.id)
            .await?
            .roleplay_sessions;

        if sessions.is_empty() {
            println!("No existing sessions found. Creating a new one.");
            return Ok(None);
        }

        let mut selection_items: Vec<String> = sessions
            .iter()
            .map(|s| {
                format!(
                    "Session {} ({} messages, created at: {})",
                    s.id,
                    s.roleplay_messages.len(),
                    s.created_at.format("%Y-%m-%d %H:%M")
                )
            })
            .collect();
        selection_items.push("Create a new session".to_string());
        selection_items.push("Exit".to_string());

        let selection = tokio::task::spawn_blocking(move || {
            Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select a session to continue, or create a new one")
                .default(0)
                .items(&selection_items)
                .interact()
        })
        .await??;

        match selection {
            i if i < sessions.len() => Ok(Some(sessions.into_iter().nth(i).unwrap())),
            i if i == sessions.len() => Ok(None),
            _ => {
                println!("{}", "Exiting.".red());
                std::process::exit(0);
            }
        }
    }

    /// Guides the user through creating a new chat session.
    async fn create_new_session(&self) -> Result<Session> {
        println!("Fetching characters...");
        let characters = self
            .state
            .graphql_client
            .get_all_characters()
            .await?
            .roleplay_characters;
        
        if characters.is_empty() {
            println!("{}", "No characters found. Please create a character first.".yellow());
            // Exit gracefully
            std::process::exit(0);
        }
        
        let character_names: Vec<String> = characters.iter().map(|c| c.name.clone()).collect();

        let char_selection = tokio::task::spawn_blocking(move || {
            Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select a character")
                .default(0)
                .items(&character_names)
                .interact()
        })
        .await??;
        let selected_character = &characters[char_selection];

        println!("Fetching system configs...");
        let system_configs = self
            .state
            .graphql_client
            .get_all_system_configs()
            .await?
            .system_configs;
        let config_names: Vec<String> = system_configs.iter().map(|c| c.name.clone()).collect();

        let config_selection = tokio::task::spawn_blocking(move || {
            Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select a system configuration")
                .default(0)
                .items(&config_names)
                .interact()
        })
        .await??;
        let selected_config = &system_configs[config_selection];

        println!("Creating new session...");
        self.state
            .api_client
            .create_session(
                selected_character.id.to_string(),
                selected_config.id.to_string(),
            )
            .await?;

        // Poll for the new session to appear
        let new_session = self.poll_for_new_session(selected_character.id).await?;

        println!("{}", "New session created!".green());
        Ok(new_session)
    }

    /// Polls the server until the newly created session is available.
    async fn poll_for_new_session(&self, character_id: Uuid) -> Result<Session> {
        loop {
            let sessions = self
                .state
                .graphql_client
                .get_my_sessions_and_messages(&self.state.user.id)
                .await?
                .roleplay_sessions;
            if let Some(session) = sessions.into_iter().find(|s| s.character == character_id) {
                return Ok(session);
            }
            sleep(Duration::from_millis(500)).await;
        }
    }

    /// Handles the main chat interaction loop for a given session.
    async fn chat_loop(&mut self, mut session: Session) -> Result<()> {
        println!("\n--- Entering Chat with Session {} ---", session.id);
        for message in &session.roleplay_messages {
            let speaker = if message.role == "user" { "You:".green() } else { "Bot:".yellow() };
            println!("{speaker}");
            self.skin.print_text(&message.content);
        }
        println!("--- (type 'exit' to end) ---");

        loop {
            let readline = self.rl.readline(&format!("{} ", "You:".green()));
            match readline {
                Ok(line) => {
                    let input = line.trim();
                    if input.is_empty() { continue; }
                    if input.eq_ignore_ascii_case("exit") { break; }

                    self.rl.add_history_entry(input)?;
                    let message_count_before = session.roleplay_messages.len();

                    let pb = create_spinner("Bot is thinking...");
                    self.state
                        .api_client
                        .chat(session.id.to_string(), input.to_string())
                        .await?;

                    // Poll for new messages
                    let updated_session = self.poll_for_new_messages(session.id, message_count_before).await?;
                    let new_messages = updated_session.roleplay_messages;
                    
                    pb.finish_and_clear();

                    // Display only new assistant messages
                    for message in new_messages.iter().skip(message_count_before) {
                        if message.role != "user" {
                            println!("{}:", "Bot:".yellow());
                            self.skin.print_text(&message.content);
                        }
                    }
                    session.roleplay_messages = new_messages;
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    println!("{}", "\nExiting session.".red());
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

    /// Polls the server until new messages are received for the session.
    async fn poll_for_new_messages(&self, session_id: Uuid, old_count: usize) -> Result<Session> {
        loop {
            let sessions = self
                .state
                .graphql_client
                .get_my_sessions_and_messages(&self.state.user.id)
                .await?
                .roleplay_sessions;
            if let Some(updated_session) = sessions.into_iter().find(|s| s.id == session_id) {
                if updated_session.roleplay_messages.len() > old_count {
                    return Ok(updated_session);
                }
            }
            sleep(Duration::from_millis(200)).await;
        }
    }
}

/// Creates a default skin for styling terminal output.
fn create_skin() -> MadSkin {
    let mut skin = MadSkin::default_dark();
    skin.bold.set_fg(Color::Rgb { r: 255, g: 255, b: 85 });
    skin.italic.set_fg(Color::AnsiValue(245));
    skin.italic.add_attr(Attribute::Italic);
    skin.bullet = termimad::StyledChar::from_fg_char(Color::Rgb { r: 255, g: 215, b: 0 }, '❯');
    skin.paragraph.set_fg(Color::AnsiValue(252));
    skin
}

/// Creates and configures a new `ProgressBar` spinner.
fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG) // Set to INFO for cleaner output
        .init();

    println!("{}", "Sandbox CLI initializing...".bold());
    
    let mut app = App::new()?;
    if let Err(e) = app.run().await {
        eprintln!("{} {:#}", "Application error:".red().bold(), e);
        std::process::exit(1);
    }
    
    Ok(())
}