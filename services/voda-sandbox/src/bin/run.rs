use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use rustyline::{error::ReadlineError, DefaultEditor};
use termimad::{crossterm::style::Attribute, MadSkin};
use termimad::crossterm::style::Color;
use tokio::time::sleep;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use voda_database::{init_db_pool, QueryCriteria, SqlxCrud, SqlxFilterQuery};
use voda_runtime::User;

use voda_sandbox::{
    api_client::ApiClient,
    config::get_normal_user,
    graphql::{CharacterSummary, GraphQlClient, Session, SystemConfig},
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
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const POLL_TIMEOUT: Duration = Duration::from_secs(30);

struct AppState {
    api_client: ApiClient,
    graphql_client: GraphQlClient,
}

struct Tui {
    rl: Arc<Mutex<DefaultEditor>>,
    skin: MadSkin,
}

impl Tui {
    fn new() -> Result<Self> {
        Ok(Self {
            rl: Arc::new(Mutex::new(DefaultEditor::new()?)),
            skin: Self::create_skin(),
        })
    }

    #[instrument(skip(self, items))]
    async fn select_item<S: ToString + std::fmt::Debug>(
        &self,
        prompt: &str,
        items: &[S],
    ) -> Result<usize> {
        let theme = ColorfulTheme::default();
        let owned_items: Vec<String> = items.iter().map(|s| s.to_string()).collect();
        let prompt_owned = prompt.to_string();

        let selection = tokio::task::spawn_blocking(move || {
            Select::with_theme(&theme)
                .with_prompt(&prompt_owned)
                .default(0)
                .items(&owned_items)
                .interact()
        })
        .await?
        .context("User did not make a selection")?;
        Ok(selection)
    }

    #[instrument(skip(self))]
    fn get_user_input(&self, prompt: &str) -> Result<String, ReadlineError> {
        let rl_clone = Arc::clone(&self.rl);
        tokio::task::block_in_place(move || {
            let mut rl = rl_clone.lock().unwrap();
            let line = rl.readline(prompt)?;
            if !line.trim().is_empty() {
                rl.add_history_entry(line.as_str().trim())?;
            }
            Ok(line)
        })
    }

    fn print_history(&self, session: &Session) {
        println!("\n--- Chat History for Session {} ---", session.id);
        for message in &session.roleplay_messages {
            if message.role == "user" {
                self.print_user_message(&message.content);
            } else {
                self.print_bot_message(&message.content);
            }
        }
        println!("--- (type '/exit' or '/quit' to end session) ---");
    }

    fn print_user_message(&self, content: &str) {
        println!("{}", "You:".green());
        self.skin.print_text(content);
    }

    fn print_bot_message(&self, content: &str) {
        println!("{}", "Bot:".yellow());
        self.skin.print_text(content);
    }

    fn create_spinner(&self, msg: &str) -> ProgressBar {
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

    fn create_skin() -> MadSkin {
        let mut skin = MadSkin::default_dark();
        skin.bold.set_fg(Color::Rgb { r: 255, g: 255, b: 85 });
        skin.italic.set_fg(Color::AnsiValue(245));
        skin.italic.add_attr(Attribute::Italic);
        skin.bullet = termimad::StyledChar::from_fg_char(Color::Rgb { r: 255, g: 215, b: 0 }, '❯');
        skin.paragraph.set_fg(Color::AnsiValue(252));
        skin
    }
}

async fn get_or_create_user(pool: &sqlx::PgPool) -> Result<User> {
    let template_user = get_normal_user();
    let user_id = template_user.user_id.clone();
    let criteria = QueryCriteria::new()
        .add_filter("user_id", "=", Some(user_id))?
        .limit(1)?;

    match User::find_one_by_criteria(criteria, pool).await? {
        Some(user) => {
            info!(
                "Found existing user in DB: {} ({})",
                user.user_aka.cyan(),
                user.id
            );
            Ok(user)
        }
        None => {
            info!("User not found, creating a new one in the database...");
            let new_user = template_user.create(pool).await?;
            info!(
                "Created new user in DB: {} ({})",
                new_user.user_aka.cyan(),
                new_user.id
            );
            Ok(new_user)
        }
    }
}

struct App {
    state: AppState,
    tui: Tui,
}

impl App {
    async fn new(pool: &sqlx::PgPool) -> Result<Self> {
        let user = get_or_create_user(pool).await?;
        info!("Welcome, {}! (ID: {})", user.user_aka.cyan(), user.id);
        let secret_key = std::env::var("SECRET_SALT").expect("SECRET_SALT must be set");

        let http_client = Client::new();
        let api_client = ApiClient::new(
            BASE_URL.to_string(),
            user.clone(),
            secret_key.clone(),
            http_client.clone(),
        );
        let graphql_client =
            GraphQlClient::new(BASE_URL.to_string(), user.clone(), secret_key, http_client);

        Ok(Self {
            state: AppState {
                api_client,
                graphql_client,
            },
            tui: Tui::new()?,
        })
    }

    #[instrument(skip(self))]
    async fn run(&mut self) -> Result<()> {
        loop {
            match self.select_or_create_session().await {
                Ok(session) => {
                    if let Err(e) = self.chat_loop(session).await {
                        error!("Chat loop ended with error: {:#}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to select or create session: {:#}", e);
                    println!("{}", "Could not start a session. Please try again.".red());
                    sleep(Duration::from_secs(1)).await;
                }
            }
            println!("\n{}\n", "Returning to session selection...".bold());
        }
    }

    #[instrument(skip(self))]
    async fn select_or_create_session(&self) -> Result<Session> {
        info!("Starting session selection process");
        let sessions = self.fetch_sessions().await?;
        if sessions.is_empty() {
            println!("No existing sessions found.");
            return self.create_new_session().await;
        }

        match self.select_from_existing_sessions(&sessions).await? {
            Some(session) => {
                info!("Selected existing session {}", session.id);
                Ok(session)
            },
            None => {
                info!("User opted to create a new session");
                self.create_new_session().await
            }
        }
    }

    #[instrument(skip(self))]
    async fn fetch_sessions(&self) -> Result<Vec<Session>> {
        let pb = self.tui.create_spinner("Fetching sessions...");
        let sessions = self.state.graphql_client.get_my_sessions_and_messages().await?.roleplay_sessions;
        pb.finish_and_clear();
        Ok(sessions)
    }

    #[instrument(skip(self, sessions))]
    async fn select_from_existing_sessions(&self, sessions: &[Session]) -> Result<Option<Session>> {
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

        let selection = self.tui.select_item("Select a session", &selection_items).await?;

        match selection {
            i if i < sessions.len() => Ok(Some(sessions[i].clone())),
            i if i == sessions.len() => Ok(None),
            _ => {
                println!("{}", "Exiting.".red());
                std::process::exit(0);
            }
        }
    }

    #[instrument(skip(self))]
    async fn create_new_session(&self) -> Result<Session> {
        println!("Starting new session creation...");
        let character = self.select_character().await?;
        let system_config = self.select_system_config().await?;

        let pb = self.tui.create_spinner("Creating session on the server...");
        self.state.api_client
            .create_session(character.id.to_string(), system_config.id.to_string())
            .await
            .context("API call to create session failed")?;
        pb.finish_with_message("Session creation initiated. Waiting for it to be ready...");

        let new_session = self.poll_for_new_session(character.id).await?;
        println!("{}", "New session created successfully!".green());
        info!("Created new session {}", new_session.id);
        Ok(new_session)
    }
    
    #[instrument(skip(self))]
    async fn select_character(&self) -> Result<CharacterSummary> {
        let pb = self.tui.create_spinner("Fetching characters...");
        let characters = self
            .state
            .graphql_client
            .get_all_characters()
            .await?
            .roleplay_characters;
        pb.finish_and_clear();

        if characters.is_empty() {
            return Err(anyhow!(
                "No characters found on the server. Please create one first."
            ));
        }

        let character_names: Vec<String> = characters.iter().map(|c| c.name.clone()).collect();
        let selection = self
            .tui
            .select_item("Select a character", &character_names)
            .await?;
        Ok(characters[selection].clone())
    }

    #[instrument(skip(self))]
    async fn select_system_config(&self) -> Result<SystemConfig> {
        let pb = self.tui.create_spinner("Fetching system configurations...");
        let configs = self.state.graphql_client.get_all_system_configs().await?.system_configs;
        pb.finish_and_clear();

        if configs.is_empty() {
            return Err(anyhow!("No system configs found on the server."));
        }

        let config_names: Vec<String> = configs.iter().map(|c| c.name.clone()).collect();
        let selection = self.tui.select_item("Select a system configuration", &config_names).await?;
        Ok(configs[selection].clone())
    }

    #[instrument(skip_all, fields(session_id = %session.id))]
    async fn chat_loop(&mut self, mut session: Session) -> Result<()> {
        self.tui.print_history(&session);

        loop {
            let prompt = format!("{} ", "You:".green());
            match self.tui.get_user_input(&prompt) {
                Ok(line) => {
                    let input = line.trim();
                    if input.is_empty() {
                        continue;
                    }
                    if self.handle_command(input, &mut session).await? {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    println!("{}", "\nExiting session.".red());
                    break;
                }
                Err(err) => {
                    error!("Readline error: {:?}", err);
                    return Err(err.into());
                }
            }
        }
        info!("Exiting chat loop for session {}", session.id);
        Ok(())
    }

    #[instrument(skip(self, session))]
    async fn handle_command(&self, input: &str, session: &mut Session) -> Result<bool> {
        match input.to_lowercase().as_str() {
            "/exit" | "/quit" => return Ok(true),
            "/rollback" => {
                info!("Performing rollback for session {}", session.id);
                let pb = self.tui.create_spinner("Rolling back last message...");
                let message_count_before = session.roleplay_messages.len();
                
                self.state.api_client.rollback(session.id.to_string(), "".to_string()).await?;
                
                let updated_session = self.poll_for_message_change(session.id, message_count_before).await?;
                pb.finish_with_message("Rollback successful.");
                
                *session = updated_session;
                self.tui.print_history(session);
            },
            _ => {
                let pb = self.tui.create_spinner("Bot is thinking...");
                let message_count_before = session.roleplay_messages.len();

                self.state.api_client.chat(session.id.to_string(), input.to_string()).await?;

                let updated_session = self.poll_for_message_change(session.id, message_count_before).await?;
                let new_messages = &updated_session.roleplay_messages[message_count_before..];

                pb.finish_and_clear();
                
                for message in new_messages {
                    if message.role != "user" {
                        self.tui.print_bot_message(&message.content);
                    }
                }
                *session = updated_session;
            }
        }
        Ok(false)
    }

    #[instrument(skip(self))]
    async fn poll_for_new_session(&self, character_id: Uuid) -> Result<Session> {
        info!("Polling for new session with character_id: {}", character_id);
        let start_time = tokio::time::Instant::now();
        loop {
            if start_time.elapsed() > POLL_TIMEOUT {
                return Err(anyhow!("Timeout waiting for new session to appear."));
            }
            let mut sessions = self
                .state
                .graphql_client
                .get_session_by_character(character_id)
                .await?
                .roleplay_sessions;

            if let Some(session) = sessions.pop() {
                info!("Found new session {}", session.id);
                return Ok(session);
            }
            debug!(
                "Session not found yet, polling again in {:?}...",
                POLL_INTERVAL
            );
            sleep(POLL_INTERVAL).await;
        }
    }

    #[instrument(skip(self))]
    async fn poll_for_message_change(&self, session_id: Uuid, old_count: usize) -> Result<Session> {
        info!("Polling for message changes in session {}, old count: {}", session_id, old_count);
        let start_time = tokio::time::Instant::now();
        loop {
            if start_time.elapsed() > POLL_TIMEOUT {
                return Err(anyhow!("Timeout waiting for new messages."));
            }
            let sessions = self.state.graphql_client.get_my_sessions_and_messages().await?.roleplay_sessions;
            if let Some(updated_session) = sessions.into_iter().find(|s| s.id == session_id) {
                if updated_session.roleplay_messages.len() != old_count {
                    info!("Found {} new/changed messages in session {}", updated_session.roleplay_messages.len() as i64 - old_count as i64, session_id);
                    return Ok(updated_session);
                }
            }
            debug!("No message change detected, polling again in {:?}...", POLL_INTERVAL);
            sleep(POLL_INTERVAL).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // tracing_subscriber::fmt()
    //     .with_max_level(tracing::Level::INFO)
    //     .with_target(true)
    //     .with_line_number(true)
    //     .init();

    info!("{}", "Sandbox CLI initializing...".bold());
    let pool = connect(false, false).await;
    info!("Database pool initialized.");

    match App::new(pool).await {
        Ok(mut app) => {
            if let Err(e) = app.run().await {
                error!(
                    "{} {:#}",
                    "Application exited with a critical error:".red().bold(),
                    e
                );
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!(
                "{} {:#}",
                "Failed to initialize application:".red().bold(),
                e
            );
            std::process::exit(1);
        }
    }

    Ok(())
}