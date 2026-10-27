//! User-global `SystemPrompt.md` support.
//!
//! Loads `%APPDATA%\Zed\SystemPrompt.md` (or the platform equivalent, next to
//! `AGENTS.md`) into an in-memory global and watches it for changes. When the
//! file exists and is non-empty, its contents are rendered as a Handlebars
//! template in place of the system prompt template embedded in Zed. If the
//! file is missing, empty, or unreadable, the embedded template is used.
//!
//! On startup, if the file does not exist it is created with a copy of the
//! built-in template, so the prompt is externalized and editable out of the
//! box.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use fs::Fs;
use futures::StreamExt as _;
use gpui::{App, AppContext as _, BorrowAppContext, Global, SharedString, Task};
use settings::watch_config_file;

use crate::templates::default_system_prompt_template;

/// In-memory state of the user-global `SystemPrompt.md` file.
#[derive(Debug, Default, Clone)]
pub enum SystemPromptState {
    /// The file is missing, empty, or whitespace-only; the built-in prompt is used.
    #[default]
    Empty,
    /// The file was loaded successfully; carries its trimmed contents.
    Loaded(SharedString),
    /// The file exists but could not be read; carries the error message.
    Error(SharedString),
}

impl SystemPromptState {
    /// The trimmed `SystemPrompt.md` contents, if the file was loaded successfully.
    pub fn content(&self) -> Option<&SharedString> {
        match self {
            Self::Loaded(content) => Some(content),
            Self::Empty | Self::Error(_) => None,
        }
    }

    /// The most recent read error, if the file exists but could not be read.
    pub fn error(&self) -> Option<&SharedString> {
        match self {
            Self::Error(message) => Some(message),
            Self::Empty | Self::Loaded(_) => None,
        }
    }
}

/// Global wrapper that owns the current [`SystemPromptState`] plus the watcher
/// task responsible for keeping it up to date.
pub struct SystemPrompt {
    state: SystemPromptState,
    _watcher: Task<()>,
}

impl Global for SystemPrompt {}

impl SystemPrompt {
    pub fn global(cx: &App) -> Option<&Self> {
        cx.try_global::<SystemPrompt>()
    }

    pub fn state(&self) -> &SystemPromptState {
        &self.state
    }

    /// Convenience accessor for the trimmed `SystemPrompt.md` contents.
    pub fn content(&self) -> Option<&SharedString> {
        self.state.content()
    }

    /// Convenience accessor for the most recent read error.
    pub fn error(&self) -> Option<&SharedString> {
        self.state.error()
    }
}

/// Initialize the user-global `SystemPrompt.md` watcher.
///
/// Creates the file from the built-in template when it is missing, then watches
/// [`paths::system_prompt_file`] for changes and updates the [`SystemPrompt`]
/// global. The `on_change` callback runs on the foreground thread whenever a
/// new read completes.
///
/// Calling this more than once replaces the previous global, which drops the
/// previous watcher task and cancels it.
pub fn init(
    fs: Arc<dyn Fs>,
    cx: &mut App,
    on_change: impl Fn(&SystemPromptState, &mut App) + 'static,
) {
    let path = paths::system_prompt_file().clone();
    let watcher = cx.spawn(async move |cx| {
        if let Err(error) = create_default_if_missing(fs.as_ref(), &path).await {
            log::error!(
                "Failed to create {}: {error:#}",
                paths::GLOBAL_SYSTEM_PROMPT_FILE_DISPLAY
            );
        }

        let (mut rx, watcher_task) =
            watch_config_file(cx.background_executor(), fs.clone(), path.clone());
        // Keep the file watcher task alive for as long as this task runs.
        let _watcher_task = watcher_task;

        while let Some(raw) = rx.next().await {
            let trimmed = raw.trim();
            let new_state = if !trimmed.is_empty() {
                SystemPromptState::Loaded(SharedString::from(trimmed.to_string()))
            } else if let Some(error) = probe_read_error(fs.as_ref(), &path).await {
                SystemPromptState::Error(error)
            } else {
                SystemPromptState::Empty
            };

            cx.update(|cx| {
                cx.update_global::<SystemPrompt, _>(|prompt, _| {
                    prompt.state = new_state.clone();
                });
                on_change(&new_state, cx);
            });
        }
    });

    cx.set_global(SystemPrompt {
        state: SystemPromptState::default(),
        _watcher: watcher,
    });
}

async fn create_default_if_missing(fs: &dyn Fs, path: &Path) -> Result<()> {
    if fs.metadata(path).await?.is_some() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs.create_dir(parent).await?;
    }

    let template = default_system_prompt_template();
    fs.write(path, template.as_bytes()).await?;
    log::info!(
        "Created user system prompt at {}",
        paths::GLOBAL_SYSTEM_PROMPT_FILE_DISPLAY
    );
    Ok(())
}

async fn probe_read_error(fs: &dyn Fs, path: &Path) -> Option<SharedString> {
    match fs.load(path).await {
        Ok(_) => None,
        Err(err) => {
            if let Some(io_err) = err.downcast_ref::<std::io::Error>()
                && io_err.kind() == std::io::ErrorKind::NotFound
            {
                return None;
            }
            Some(SharedString::from(format!("{err:#}")))
        }
    }
}
