use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use codeburn_pricing::PricingMetrics;
use codeburn_provider::SessionData;
use serde_json::json;

pub enum TuiOutput {
    Json(serde_json::Value),
    Dashboard(TuiDashboard),
    Status(TuiStatus),
}

const ASCII_FILL: &str = "████████░░";
const PANEL_SEPARATOR: &str = "─";
const PERIOD_OPTIONS: &[&str] = &["today", "7_days", "30_days", "month", "all"];

pub struct TuiDashboard {
    metrics: PricingMetrics,
    sessions: Vec<SessionData>,
    period: String,
}

impl TuiDashboard {
    pub fn new(metrics: PricingMetrics, sessions: Vec<SessionData>, period: String) -> Self {
        Self {
            metrics,
            sessions,
            period,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let header_height = 3;
        let footer_height = 1;
        let panel_area = Rect::new(
            area.x,
            area.y + header_height,
            area.width,
            area.height - header_height - footer_height,
        );

        self.render_header(frame, area);
        self.render_panels(frame, panel_area);
        self.render_footer(frame, area);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let period_line = Line::from(
            PERIOD_OPTIONS
                .iter()
                .map(|p| {
                    if *p == self.period {
                        Span::raw(format!(" [{}] ", p)).fg(Color::Yellow).bold()
                    } else {
                        Span::raw(format!(" {} ", p)).fg(Color::White)
                    }
                })
                .collect::<Vec<Span>>(),
        );

        let title_line = Line::from("codeburn report").fg(Color::Cyan).bold();

        let header_rect = Rect::new(area.x, area.y, area.width, 3);

        let layout = Layout::new(
            Direction::Vertical,
            [Constraint::Length(1), Constraint::Length(1)],
        )
        .split(header_rect);

        frame.render_widget(Paragraph::new(title_line), layout[0]);
        frame.render_widget(Paragraph::new(period_line), layout[1]);
    }

    fn render_panels(&self, frame: &mut Frame, area: Rect) {
        let panels = Layout::new(Direction::Vertical, [Constraint::Ratio(1, 9); 9]).split(area);

        self.render_overview(frame, panels[0]);
        self.render_daily(frame, panels[1]);
        self.render_projects(frame, panels[2]);
        self.render_top_sessions(frame, panels[3]);
        self.render_activities(frame, panels[4]);
        self.render_models(frame, panels[5]);
        self.render_tools(frame, panels[6]);
        self.render_shell(frame, panels[7]);
        self.render_mcp(frame, panels[8]);

        for i in 0..8 {
            let separator_rect = Rect::new(
                panels[i].x,
                panels[i].y + panels[i].height,
                panels[i].width,
                1,
            );
            frame.render_widget(
                Paragraph::new(Line::from(PANEL_SEPARATOR.repeat(panels[i].width as usize))),
                separator_rect,
            );
        }
    }

    fn render_overview(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("Overview"))
            .fg(Color::White);

        let overview_content = self.build_overview_lines();

        let inner = area.inner(Margin::new(1, 1));

        let layout = Layout::new(Direction::Vertical, [Constraint::Ratio(1, 4); 4]).split(inner);

        frame.render_widget(Paragraph::new(overview_content[0].clone()), layout[0]);
        frame.render_widget(Paragraph::new(overview_content[1].clone()), layout[1]);
        frame.render_widget(Paragraph::new(overview_content[2].clone()), layout[2]);
        frame.render_widget(Paragraph::new(overview_content[3].clone()), layout[3]);

        frame.render_widget(block, area);
    }

    fn build_overview_lines(&self) -> [Line<'static>; 4] {
        let total_cost = self.metrics.total_cost;
        let total_sessions = self.sessions.len();
        let cache_hit = self.metrics.efficiency;

        let token_in = self.sessions.iter().map(|s| s.input_tokens).sum::<u64>();
        let token_out = self.sessions.iter().map(|s| s.output_tokens).sum::<u64>();

        [
            Line::from(vec![
                Span::raw("cost: "),
                Span::raw(format!("{:.2}", total_cost)).fg(Color::Yellow),
                Span::raw("  calls: "),
                Span::raw(total_sessions.to_string()).fg(Color::White),
                Span::raw("  cache: "),
                Span::raw(format!("{:.0}%", cache_hit * 100.0)).fg(Color::Green),
            ]),
            Line::from(vec![
                Span::raw("tokens in: "),
                Span::raw(token_in.to_string()).fg(Color::Blue),
                Span::raw("  out: "),
                Span::raw(token_out.to_string()).fg(Color::Red),
            ]),
            Line::from(vec![
                Span::raw("period: "),
                Span::raw(self.period.to_string()).fg(Color::Cyan),
            ]),
            Line::from(vec![
                Span::raw("efficiency: "),
                Span::raw(format!("{:.0}%", cache_hit * 100.0)).fg(Color::Green),
            ]),
        ]
    }

    fn render_daily(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("Daily Activity"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let daily_lines = self.build_daily_lines();

        let layout = Layout::new(Direction::Vertical, [Constraint::Min(1); 6]).split(inner);

        for (i, line) in daily_lines.iter().enumerate() {
            frame.render_widget(Paragraph::new(line.clone()), layout[i]);
        }

        frame.render_widget(block, area);
    }

    fn build_daily_lines(&self) -> Vec<Line<'static>> {
        let max_cost = self
            .metrics
            .daily
            .iter()
            .map(|(_, c)| c)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&0.0);

        self.metrics
            .daily
            .iter()
            .rev()
            .take(6)
            .map(|(date, cost)| {
                let bar = self.render_ascii_bar(*cost, *max_cost);
                Line::from(vec![
                    Span::raw(format!("{}", date)).fg(Color::White),
                    Span::raw(" "),
                    Span::raw(bar),
                    Span::raw(" "),
                    Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                ])
            })
            .collect()
    }

    fn render_projects(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("By Project"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let project_lines = self.build_project_lines();

        let layout = Layout::new(Direction::Vertical, [Constraint::Min(1); 10]).split(inner);

        for (i, line) in project_lines.iter().enumerate() {
            frame.render_widget(Paragraph::new(line.clone()), layout[i]);
        }

        frame.render_widget(block, area);
    }

    fn build_project_lines(&self) -> Vec<Line<'static>> {
        let max_cost = self
            .metrics
            .by_project
            .values()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        self.metrics
            .by_project
            .iter()
            .map(|(name, cost)| {
                let bar = self.render_ascii_bar(*cost, max_cost);
                Line::from(vec![
                    Span::raw(name.to_string()).fg(Color::White),
                    Span::raw(" "),
                    Span::raw(bar),
                    Span::raw(" "),
                    Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                ])
            })
            .collect()
    }

    fn render_top_sessions(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("Top Sessions"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let session_lines = self.build_top_session_lines();

        let layout = Layout::new(Direction::Vertical, [Constraint::Min(1); 5]).split(inner);

        for (i, line) in session_lines.iter().enumerate() {
            frame.render_widget(Paragraph::new(line.clone()), layout[i]);
        }

        frame.render_widget(block, area);
    }

    fn build_top_session_lines(&self) -> Vec<Line<'static>> {
        let max_cost = self
            .metrics
            .top_sessions
            .iter()
            .map(|(_, c)| c)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&0.0);

        self.metrics
            .top_sessions
            .iter()
            .take(5)
            .map(|(session_id, cost)| {
                let bar = self.render_ascii_bar(*cost, *max_cost);
                Line::from(vec![
                    Span::raw(session_id.to_string()).fg(Color::White),
                    Span::raw(" "),
                    Span::raw(bar),
                    Span::raw(" "),
                    Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                ])
            })
            .collect()
    }

    fn render_activities(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("By Activity"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let activity_lines = self.build_activity_lines();

        let layout = Layout::new(Direction::Vertical, [Constraint::Min(1); 12]).split(inner);

        for (i, line) in activity_lines.iter().enumerate() {
            frame.render_widget(Paragraph::new(line.clone()), layout[i]);
        }

        frame.render_widget(block, area);
    }

    fn build_activity_lines(&self) -> Vec<Line<'static>> {
        let max_cost = self
            .metrics
            .by_activity
            .values()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        self.metrics
            .by_activity
            .iter()
            .map(|(name, cost)| {
                let bar = self.render_ascii_bar(*cost, max_cost);
                Line::from(vec![
                    Span::raw(name.to_string()).fg(Color::White),
                    Span::raw(" "),
                    Span::raw(bar),
                    Span::raw(" "),
                    Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                ])
            })
            .collect()
    }

    fn render_models(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("By Model"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let model_lines = self.build_model_lines();

        let layout = Layout::new(Direction::Vertical, [Constraint::Min(1); 10]).split(inner);

        for (i, line) in model_lines.iter().enumerate() {
            frame.render_widget(Paragraph::new(line.clone()), layout[i]);
        }

        frame.render_widget(block, area);
    }

    fn build_model_lines(&self) -> Vec<Line<'static>> {
        let max_cost = self
            .metrics
            .by_model
            .values()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        self.metrics
            .by_model
            .iter()
            .map(|(name, cost)| {
                let bar = self.render_ascii_bar(*cost, max_cost);
                Line::from(vec![
                    Span::raw(name.to_string()).fg(Color::White),
                    Span::raw(" "),
                    Span::raw(bar),
                    Span::raw(" "),
                    Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                ])
            })
            .collect()
    }

    fn render_tools(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("Core Tools"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let tool_lines = self.build_tool_lines();

        let layout = Layout::new(Direction::Vertical, [Constraint::Min(1); 10]).split(inner);

        for (i, line) in tool_lines.iter().enumerate() {
            frame.render_widget(Paragraph::new(line.clone()), layout[i]);
        }

        frame.render_widget(block, area);
    }

    fn build_tool_lines(&self) -> Vec<Line<'static>> {
        let max_calls = self
            .sessions
            .iter()
            .map(|s| s.input_tokens)
            .max()
            .unwrap_or(0_u64);

        self.metrics
            .by_tool
            .iter()
            .map(|(name, cost)| {
                let bar = self.render_ascii_bar(*cost, max_calls as f64);
                Line::from(vec![
                    Span::raw(name.to_string()).fg(Color::White),
                    Span::raw(" "),
                    Span::raw(bar),
                    Span::raw(" "),
                    Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                ])
            })
            .collect()
    }

    fn render_shell(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("Shell Commands"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let shell_lines = self.build_shell_lines();

        let layout = Layout::new(Direction::Vertical, [Constraint::Min(1); 10]).split(inner);

        for (i, line) in shell_lines.iter().enumerate() {
            frame.render_widget(Paragraph::new(line.clone()), layout[i]);
        }

        frame.render_widget(block, area);
    }

    fn build_shell_lines(&self) -> Vec<Line<'static>> {
        let max_cost = self
            .metrics
            .by_shell
            .values()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        self.metrics
            .by_shell
            .iter()
            .map(|(name, cost)| {
                let bar = self.render_ascii_bar(*cost, max_cost);
                Line::from(vec![
                    Span::raw(name.to_string()).fg(Color::White),
                    Span::raw(" "),
                    Span::raw(bar),
                    Span::raw(" "),
                    Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                ])
            })
            .collect()
    }

    fn render_mcp(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("MCP Servers"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let mcp_content = if self.metrics.by_mcp.is_empty() {
            Paragraph::new(Line::from("No MCP usage").fg(Color::Gray))
        } else {
            let mcp_lines = self
                .metrics
                .by_mcp
                .iter()
                .map(|(name, cost)| {
                    Line::from(vec![
                        Span::raw(name.to_string()).fg(Color::White),
                        Span::raw(" "),
                        Span::raw(format!("{:.2}", *cost)).fg(Color::Yellow),
                    ])
                })
                .collect::<Vec<Line>>();

            Paragraph::new(mcp_lines)
        };

        frame.render_widget(mcp_content, inner);
        frame.render_widget(block, area);
    }

    fn render_ascii_bar(&self, value: f64, max: f64) -> String {
        if max == 0.0 {
            return ASCII_FILL.to_string();
        }

        let ratio = value / max;
        let filled = (ratio * 10.0) as u32;
        let empty = 10 - filled;

        let mut bar = String::new();

        for _ in 0..filled {
            bar.push('█');
        }

        for _ in 0..empty {
            bar.push('░');
        }

        bar
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_rect = Rect::new(area.x, area.y + area.height - 1, area.width, 1);

        let footer_line = Line::from(PANEL_SEPARATOR.repeat(area.width as usize));

        frame.render_widget(Paragraph::new(footer_line), footer_rect);
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "success": true,
            "report": {
                "overview": self.metrics.total_cost,
                "daily_breakdown": self.metrics.daily,
                "projects": self.metrics.by_project,
                "models": self.metrics.by_model,
                "activities": self.metrics.by_activity,
                "tools": self.metrics.by_tool,
                "mcp_servers": self.metrics.by_mcp,
                "shell_commands": self.metrics.by_shell,
                "top_sessions": self.metrics.top_sessions,
            },
        })
    }
}

pub struct TuiStatus {
    today_usage: serde_json::Value,
    month_usage: serde_json::Value,
    currency: String,
}

impl TuiStatus {
    pub fn new(
        today_usage: serde_json::Value,
        month_usage: serde_json::Value,
        currency: String,
    ) -> Self {
        Self {
            today_usage,
            month_usage,
            currency,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Line::from("Status"))
            .fg(Color::White);

        let inner = area.inner(Margin::new(1, 1));

        let today_cost = self
            .today_usage
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let month_cost = self
            .month_usage
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let content = Paragraph::new(vec![
            Line::from(vec![
                Span::raw("today: "),
                Span::raw(today_cost.to_string()).fg(Color::Yellow),
                Span::raw(" "),
                Span::raw(self.currency.to_string()).fg(Color::White),
            ]),
            Line::from(vec![
                Span::raw("month: "),
                Span::raw(month_cost.to_string()).fg(Color::Yellow),
                Span::raw(" "),
                Span::raw(self.currency.to_string()).fg(Color::White),
            ]),
        ]);

        frame.render_widget(content, inner);
        frame.render_widget(block, area);
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "success": true,
            "status": {
                "today": self.today_usage.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
                "month": self.month_usage.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
                "currency": self.currency,
            },
        })
    }
}
