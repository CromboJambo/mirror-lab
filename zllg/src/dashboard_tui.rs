use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{config, dashboard, detect};

/// Run the IDE dashboard TUI. Blocks until the user presses Esc.
pub fn run_dashboard_tui() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let pt = detect::detect_project_type(&cwd);
    let cfg = config::load_config()?;
    let state = dashboard::build_dashboard(&cfg, pt);

    // Enter alternate screen.
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| {
        let size = frame.area();
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Percentage(10),
                Constraint::Percentage(60),
                Constraint::Percentage(30),
            ])
            .split(size);

        // Header
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "zllg IDE",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — ", Style::default()),
            Span::styled(&state.project_type, Style::default().fg(Color::Yellow)),
        ]))
        .block(Block::default().borders(Borders::TOP));
        frame.render_widget(header, chunks[0]);

        // Pane status
        let pane_lines: Vec<Line> = state
            .panes
            .iter()
            .map(|p| {
                let mark = if p.visible {
                    Span::styled("◉", Style::default().fg(Color::Green))
                } else {
                    Span::styled("○", Style::default().fg(Color::Red))
                };
                let embed = if p.embedded {
                    Span::styled("in", Style::default().fg(Color::Blue))
                } else {
                    Span::styled("out", Style::default().fg(Color::Magenta))
                };
                let name = p.name.clone();
                let idx = p.index.to_string();
                Line::from(vec![
                    mark,
                    Span::styled(" ", Style::default()),
                    Span::styled(name, Style::default().fg(Color::White)),
                    Span::styled(" ", Style::default()),
                    embed,
                    Span::styled(" ", Style::default()),
                    Span::styled(idx, Style::default().fg(Color::Gray)),
                ])
            })
            .collect();
        let pane_widget =
            Paragraph::new(pane_lines).block(Block::default().borders(Borders::ALL).title("Panes"));
        frame.render_widget(pane_widget, chunks[1]);

        // Footer
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to exit", Style::default().fg(Color::Gray)),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(footer, chunks[2]);
    })?;

    // Leave alternate screen.
    execute!(std::io::stdout(), LeaveAlternateScreen)?;

    // Wait for Esc.
    loop {
        if let Event::Key(key) = event::read()?
            && key.code == KeyCode::Esc
        {
            break;
        }
    }

    Ok(())
}
