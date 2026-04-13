use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
};

use crate::{app::hint_line, config::Palette};

use super::{bindings::bindings, state::State};

fn summary(generator: &State, palette: &Palette) -> Vec<Line<'static>> {
    let rows = vec![
        (
            "Mode",
            generator.settings.mode.clone(),
            generator.selected_index == 0,
            false,
        ),
        (
            "Length",
            if generator.editing_length {
                generator.length_buffer.clone()
            } else {
                generator.settings.length.to_string()
            },
            generator.selected_index == 1,
            generator.editing_length,
        ),
        (
            "Nonconfusables",
            if generator.settings.nonconfusables {
                "on".to_string()
            } else {
                "off".to_string()
            },
            generator.selected_index == 2,
            false,
        ),
    ];
    rows.into_iter()
        .map(|(label, value, selected, editing)| {
            let prefix = if selected { "> " } else { "  " };
            let style = if editing || (selected && label == "Length") {
                Style::default()
                    .fg(palette.selected_fg)
                    .bg(palette.selected_bg)
            } else if selected {
                Style::default().fg(palette.accent)
            } else {
                Style::default().fg(palette.text)
            };
            Line::from(vec![
                Span::raw(prefix),
                Span::raw(format!("{label}: ")),
                Span::styled(value, style),
            ])
        })
        .collect()
}

fn help_text(state: &State) -> &'static str {
    if state.editing_length {
        "Type digits, Backspace deletes, Enter/Esc confirms"
    } else if state.selected_index == 0 {
        match state.settings.mode.as_str() {
            "standard" => "standard: mixed letters, numbers, symbols",
            "no-symbols" => "no-symbols: letters and numbers only",
            "diceware" => "diceware: multiple random words",
            "numeric" => "numeric: digits only",
            _ => "Mode",
        }
    } else if state.selected_index == 1 {
        "Length ctrls password size, or word count in diceware mode"
    } else {
        "Nonconfusables removes visually similar characters when supported"
    }
}

pub(crate) fn render_modal(frame: &mut ratatui::Frame<'_>, palette: &Palette, generator: &State) {
    let inner = crate::app::render_popup_shell(frame, 60, 11, "Generate Password", palette.accent);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);
    let summary = Paragraph::new(Text::from(summary(generator, palette)));
    frame.render_widget(summary, rows[0]);
    let keys = Paragraph::new(Text::from(vec![hint_line(bindings(generator), palette)]));
    frame.render_widget(keys, rows[1]);
    let help = Paragraph::new(help_text(generator)).style(Style::default().fg(palette.help));
    frame.render_widget(help, rows[2]);
}

/// Returns the cursor position while editing generator length.
pub(crate) fn cursor_position(area: Rect, generator: &State) -> (u16, u16) {
    let popup = crate::app::popup_area(area, 60, 11);
    let block_inner = popup.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let x = block_inner.x
        + "  Length: ".chars().count() as u16
        + generator.length_buffer.chars().count() as u16;
    let y = block_inner.y + 1;
    (x.min(popup.right().saturating_sub(2)), y)
}
