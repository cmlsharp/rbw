use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    time::Duration,
};

use crossterm::{
    ExecutableCommand,
    cursor::{self, SetCursorStyle},
    event::{
        self, Event, KeyEventKind, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
};
use ratatui::{
    DefaultTerminal,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    browser,
    clipboard::DEFAULT_CLIPBOARD_TIMEOUT_SECONDS,
    config::{AppConfig, Palette},
    create,
    domain::{Entry, Scope},
    generator, rbw,
};

mod effects;
mod input;
mod reducer;

use self::{input::map_key_to_actions, reducer::reduce};

pub(crate) use self::{
    effects::{Effect, EffectOutcome, EffectResult},
    input::{
        Binding, StaticLabel, lookup_action, lookup_action_with_fallback,
    },
    reducer::{Action, SystemAction, Transition},
};

use ratatui::{
    style::Modifier,
    text::{Line, Span},
};

/// Renders one hint line with colored bindings and labels.
pub(crate) fn hint_line<'a, A: StaticLabel + 'a>(
    bindings: impl IntoIterator<Item = &'a Binding<A>>,
    palette: &Palette,
) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, binding) in bindings
        .into_iter()
        .filter_map(|binding| {
            binding
                .hint_key()
                .map(|key| (key, binding.action().label()))
        })
        .enumerate()
    {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            binding.0.to_string(),
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            binding.1.to_string(),
            Style::default().fg(palette.text),
        ));
    }
    Line::from(spans)
}

pub const NOTIFICATION_TIMEOUT_SECONDS: u64 = 2;

// Use RAII to make sure we undo our terminal settings changes
struct AppTerminal {
    terminal: DefaultTerminal,
    keyboard_enhancement_pushed: bool,
}

impl AppTerminal {
    fn new() -> anyhow::Result<Self> {
        let terminal = ratatui::try_init()?;
        let mut terminal = Self {
            terminal,
            keyboard_enhancement_pushed: false,
        };

        if crossterm::terminal::supports_keyboard_enhancement()
            .unwrap_or_default()
        {
            terminal
                .backend_mut()
                .execute(PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES,
                ))?;
            terminal.keyboard_enhancement_pushed = true;
        }

        Ok(terminal)
    }
}

impl Deref for AppTerminal {
    type Target = DefaultTerminal;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for AppTerminal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for AppTerminal {
    fn drop(&mut self) {
        if self.keyboard_enhancement_pushed {
            let _ = self
                .terminal
                .backend_mut()
                .execute(PopKeyboardEnhancementFlags);
        }
        let _ = self.terminal.backend_mut().execute(cursor::Show);
        let _ = self
            .terminal
            .backend_mut()
            .execute(SetCursorStyle::DefaultUserShape);
        ratatui::restore();
    }
}

/// Immutable context passed into the browser workflow.
#[derive(Debug)]
pub struct Context {
    pub url: String,
    pub username: String,
    pub emit_output: bool,
    pub palette: Palette,
    clear_timeout_seconds: u64,
}

/// High-level modal/screen mode.
#[derive(Debug)]
pub enum Mode {
    Normal,
    Search,
    Generator(generator::State),
    Create(create::State),
    DeleteConfirm(Entry),
}

/// Top-level application state.
#[derive(Debug)]
pub struct State {
    pub context: Context,
    pub browser: browser::State,
    mode: Mode,
    notification: Option<Notification>,
    generator_settings: generator::Settings,
}

/// Floating success/error notification.
#[derive(Debug)]
pub struct Notification {
    message: String,
    created_at: std::time::Instant,
    is_error: bool,
}

impl State {
    /// Sets a transient notification.
    fn notify(&mut self, message: impl Into<String>) {
        self.set_notification(message, false);
    }

    /// Sets a transient error notification.
    fn notify_error(&mut self, message: impl Into<String>) {
        self.set_notification(message, true);
    }

    fn set_notification(
        &mut self,
        message: impl Into<String>,
        is_error: bool,
    ) {
        self.notification = Some(Notification {
            message: message.into(),
            created_at: std::time::Instant::now(),
            is_error,
        });
    }

    /// Expires the current notification if it has been visible long enough.
    fn expire_notification(&mut self, timeout_seconds: u64) {
        if self.notification.as_ref().is_some_and(|notice| {
            notice.created_at.elapsed()
                >= std::time::Duration::from_secs(timeout_seconds)
        }) {
            self.notification = None;
        }
    }
}

/// Centers a popup rectangle within a terminal area.
pub(crate) fn popup_area(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(vertical[1]);
    horizontal[1]
}

/// Returns the standard inner content area for modal popups.
pub(crate) fn popup_inner_area(area: Rect) -> Rect {
    area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    })
    .inner(Margin {
        vertical: 0,
        horizontal: 1,
    })
}

/// Renders the standard popup shell and returns the inner content area.
pub(crate) fn render_popup_shell(
    frame: &mut ratatui::Frame<'_>,
    width: u16,
    height: u16,
    title: &str,
    border_color: Color,
) -> Rect {
    let area = popup_area(frame.area(), width, height);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(block, area);
    popup_inner_area(area)
}

fn render_notification(
    frame: &mut ratatui::Frame<'_>,
    palette: &Palette,
    notification: Option<&Notification>,
) {
    let Some(notification) = notification else {
        return;
    };
    let width = notification
        .message
        .chars()
        .count()
        .saturating_add(4)
        .clamp(24, 80) as u16;
    let area = Rect {
        x: frame.area().width.saturating_sub(width + 2),
        y: 1,
        width,
        height: 3,
    };
    let border = if notification.is_error {
        palette.danger
    } else {
        palette.accent
    };
    frame.render_widget(Clear, area);
    let paragraph = Paragraph::new(notification.message.as_str())
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        )
        .style(Style::default().fg(palette.text));
    frame.render_widget(paragraph, area);
}

fn cursor_for_state(state: &State, area: Rect) -> Option<(u16, u16)> {
    match &state.mode {
        Mode::Search => Some(browser::search_cursor(area, &state.browser)),
        Mode::Generator(generator) if generator.editing_length => {
            Some(generator::cursor_position(area, generator))
        }
        Mode::Create(create) => Some(create::cursor_position(area, create)),
        _ => None,
    }
}

fn render(
    terminal: &mut AppTerminal,
    state: &State,
) -> anyhow::Result<usize> {
    let mut rows = 1;
    terminal.draw(|frame| {
        rows = browser::render(
            frame,
            &state.browser,
            &state.context,
            &state.mode,
        );
        match &state.mode {
            Mode::Generator(generator) => generator::render_modal(
                frame,
                &state.context.palette,
                generator,
            ),
            Mode::Create(create) => {
                create::render_modal(frame, &state.context.palette, create)
            }
            Mode::DeleteConfirm(entry) => browser::delete::render_confirm(
                frame,
                &state.context.palette,
                entry,
            ),
            _ => {}
        }
        render_notification(
            frame,
            &state.context.palette,
            state.notification.as_ref(),
        );
        if let Some((x, y)) = cursor_for_state(state, frame.area()) {
            frame.set_cursor_position((x, y));
        }
    })?;
    Ok(rows)
}

/// Runs the interactive terminal UI and returns the serialized result payload.
pub fn run(
    url: String,
    username: String,
    scope: Scope,
    emit_output: bool,
) -> anyhow::Result<String> {
    let (config, config_warning) = AppConfig::load_or_default();
    let palette = Palette::from_config(&config);
    rbw::ensure_unlocked()?;
    let entries = rbw::list_entries()?;
    let context = Context {
        url,
        username,
        emit_output,
        clear_timeout_seconds: config
            .clear_timeout_seconds
            .unwrap_or(DEFAULT_CLIPBOARD_TIMEOUT_SECONDS),
        palette,
    };
    let mut browser = browser::State::new(scope, entries);
    browser.refresh_visible(&context);
    let mut state = State {
        context,
        browser,
        mode: Mode::Normal,
        notification: None,
        generator_settings: generator::Settings::from_config(&config),
    };
    if let Some(warning) = config_warning {
        state.notify_error(warning);
    }

    let mut terminal = AppTerminal::new()?;

    let result = loop {
        state.expire_notification(NOTIFICATION_TIMEOUT_SECONDS);
        state.browser.ensure_selected_visible();
        state.browser.viewport_rows = render(&mut terminal, &state)?;
        let show_cursor = match &state.mode {
            Mode::Search | Mode::Create(_) => true,
            Mode::Generator(generator) => generator.editing_length,
            _ => false,
        };
        if show_cursor {
            terminal.backend_mut().execute(cursor::Show)?;
            terminal
                .backend_mut()
                .execute(SetCursorStyle::BlinkingBar)?;
        } else {
            terminal.backend_mut().execute(cursor::Hide)?;
        }
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        // this app doesn't do anything on key release
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        let mut queue = VecDeque::from(map_key_to_actions(&state, key));
        let mut maybe_output = None;
        while let Some(action) = queue.pop_front() {
            let transition = reduce(&mut state, action);
            if let Some(output) = transition.output {
                maybe_output = Some(output);
                break;
            }
            if let Some(effect) = transition.effect {
                if let Some(label) = effect.pending_label() {
                    state.notify(label);
                    render(&mut terminal, &state)?;
                }
                queue.push_back(Action::System(SystemAction::Effect(
                    effect.run(),
                )));
            }
        }

        if let Some(output) = maybe_output {
            break output;
        }
    };

    Ok(result)
}
