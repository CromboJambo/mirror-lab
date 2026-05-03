use clap::Parser;
use codeburn_lib::{Cli, CliCommand, PlanAction};
use serde_json::json;

mod tui;

use codeburn_classifier::TaskClassifier;
use codeburn_config::CodeBurnConfig;
use codeburn_pricing::PricingEngine;
use codeburn_provider::ProviderRegistry;
use ratatui::prelude::{CrosstermBackend, Terminal};
use std::io::IsTerminal;
use tui::{TuiDashboard, TuiOutput, TuiStatus};

/// Main CLI entry point for CodeBurn
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if is_help_request(&args) {
        print_json(&usage_response(true));
        return;
    }

    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(err) => {
            print_json(&error_response(&err.to_string(), true));
            std::process::exit(1);
        }
    };

    let response = match cli.command {
        Some(CliCommand::Help) => TuiOutput::Json(usage_response(true)),
        Some(CliCommand::Report { .. }) => handle_report(&cli)
            .await
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        Some(CliCommand::Status { .. }) => handle_status(&cli)
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        Some(CliCommand::Export { .. }) => handle_export(&cli)
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        Some(CliCommand::Optimize { .. }) => handle_optimize(&cli)
            .await
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        Some(CliCommand::Compare { .. }) => handle_compare(&cli)
            .await
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        Some(CliCommand::Currency { code }) => handle_currency(&code)
            .await
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        Some(CliCommand::ModelAlias { from, to }) => handle_model_alias(&from, &to)
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        Some(CliCommand::Plan { action }) => handle_plan(&action)
            .unwrap_or_else(|err| TuiOutput::Json(error_response(&err.to_string(), true))),
        None => {
            print_json(&error_response("missing command", true));
            std::process::exit(1);
        }
    };

    match response {
        TuiOutput::Json(json_val) => {
            let exit_code = json_val
                .get("success")
                .and_then(|value| value.as_bool())
                .map(|success| if success { 0 } else { 1 })
                .unwrap_or(1);

            print_json(&json_val);
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        TuiOutput::Dashboard(dashboard) => {
            if std::io::stdout().is_terminal() {
                let backend = CrosstermBackend::new(std::io::stdout());
                let mut terminal = Terminal::new(backend).unwrap();
                terminal.clear().unwrap();
                terminal.draw(|frame| dashboard.render(frame)).unwrap();
                terminal.hide_cursor().unwrap();
            } else {
                print_json(&dashboard.to_json());
            }
        }
        TuiOutput::Status(status) => {
            if std::io::stdout().is_terminal() {
                let backend = CrosstermBackend::new(std::io::stdout());
                let mut terminal = Terminal::new(backend).unwrap();
                terminal.clear().unwrap();
                terminal.draw(|frame| status.render(frame)).unwrap();
                terminal.hide_cursor().unwrap();
            } else {
                print_json(&status.to_json());
            }
        }
    }
}

fn is_help_request(args: &[String]) -> bool {
    matches!(
        args.get(1).map(String::as_str),
        Some("help" | "--help" | "-h")
    )
}

fn print_json(response: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(response).unwrap_or_else(|_| {
            "{\"success\":false,\"error\":\"failed to serialize response\"}".to_string()
        })
    );
}

async fn handle_report(cli: &Cli) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    let project_root = std::env::current_dir()?;
    let config = CodeBurnConfig::load(&project_root)?;
    let registry = ProviderRegistry::new();
    let classifier = TaskClassifier::new();
    let pricing = PricingEngine::new();

    let _providers = ProviderRegistry::discover(&project_root)?;
    let sessions = ProviderRegistry::read_sessions(&registry)?;
    let classified = classifier.classify(&sessions)?;
    let costs = pricing
        .calculate(&classified, Some(config.currency))
        .await?;

    match &cli.command {
        Some(CliCommand::Report { format, .. }) => {
            if format == "json" {
                Ok(TuiOutput::Json(json!({
                    "success": true,
                    "report": {
                        "overview": costs.total_cost,
                        "daily_breakdown": costs.daily,
                        "projects": costs.by_project,
                        "models": costs.by_model,
                        "activities": costs.by_activity,
                        "tools": costs.by_tool,
                        "mcp_servers": costs.by_mcp,
                        "shell_commands": costs.by_shell,
                        "top_sessions": costs.top_sessions,
                    },
                })))
            } else {
                Ok(TuiOutput::Dashboard(TuiDashboard::new(
                    costs,
                    sessions,
                    cli_period(cli),
                )))
            }
        }
        _ => Ok(TuiOutput::Json(json!({
            "success": true,
            "report": {
                "overview": costs.total_cost,
                "daily_breakdown": costs.daily,
                "projects": costs.by_project,
                "models": costs.by_model,
                "activities": costs.by_activity,
                "tools": costs.by_tool,
                "mcp_servers": costs.by_mcp,
                "shell_commands": costs.by_shell,
                "top_sessions": costs.top_sessions,
            },
        }))),
    }
}

fn cli_period(cli: &Cli) -> String {
    match &cli.command {
        Some(CliCommand::Report { period, .. }) => period.clone().unwrap_or("today".to_string()),
        _ => "today".to_string(),
    }
}

fn handle_status(cli: &Cli) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    let project_root = std::env::current_dir()?;
    let config = CodeBurnConfig::load(&project_root)?;
    let registry = ProviderRegistry::new();
    let _pricing = PricingEngine::new();

    let today = registry.today_usage()?;
    let month = registry.month_usage()?;

    match &cli.command {
        Some(CliCommand::Status { format, .. }) => {
            if format == "json" {
                Ok(TuiOutput::Json(json!({
                    "success": true,
                    "status": {
                        "today": today.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
                        "month": month.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
                        "currency": config.currency,
                    },
                })))
            } else {
                Ok(TuiOutput::Status(TuiStatus::new(
                    today,
                    month,
                    config.currency,
                )))
            }
        }
        _ => Ok(TuiOutput::Json(json!({
            "success": true,
            "status": {
                "today": today.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
                "month": month.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
                "currency": config.currency,
            },
        }))),
    }
}

fn handle_export(cli: &Cli) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    let project_root = std::env::current_dir()?;
    let _config = CodeBurnConfig::load(&project_root)?;
    let registry = ProviderRegistry::new();

    let export_path = project_root.join(".codeburn-export");

    if !export_path.exists() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "export directory missing marker",
        )));
    }

    let data = registry.multi_period_export()?;

    Ok(TuiOutput::Json(json!({
        "success": true,
        "export": {
            "path": export_path.to_string_lossy(),
            "format": cli.command.clone().and_then(|c| match c {
                CliCommand::Export { format, .. } => Some(format),
                _ => None,
            }),
            "periods": data.get("periods").and_then(|v| v.as_object()).cloned().unwrap_or_default(),
        },
    })))
}

async fn handle_optimize(_cli: &Cli) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    let _project_root = std::env::current_dir()?;
    let registry = ProviderRegistry::new();
    let classifier = TaskClassifier::new();

    let sessions = registry.read_sessions()?;
    let classified = classifier.classify(&sessions)?;

    let findings = optimize_engine(&classified)?;

    Ok(TuiOutput::Json(json!({
        "success": true,
        "optimize": {
            "findings": findings,
            "fixes": findings.iter().map(|f| f.model.clone()).collect::<Vec<String>>(),
        },
    })))
}

async fn handle_compare(_cli: &Cli) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    let _project_root = std::env::current_dir()?;
    let registry = ProviderRegistry::new();
    let pricing = PricingEngine::new();
    let classifier = TaskClassifier::new();

    let sessions_a = registry.provider_sessions("model_a")?;
    let sessions_b = registry.provider_sessions("model_b")?;

    let classified_a = classifier.classify(&sessions_a)?;
    let classified_b = classifier.classify(&sessions_b)?;

    let costs_a = pricing.calculate(&classified_a, None).await?;
    let costs_b = pricing.calculate(&classified_b, None).await?;

    Ok(TuiOutput::Json(json!({
        "success": true,
        "compare": {
            "performance": costs_a.total_cost,
            "efficiency": costs_a.efficiency,
            "working_style": costs_a.style,
            "model_b_performance": costs_b.total_cost,
            "model_b_efficiency": costs_b.efficiency,
            "model_b_working_style": costs_b.style,
        },
    })))
}

async fn handle_currency(code: &str) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    if !is_valid_iso4217(code) {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("currency invalid: {code}"),
        )));
    }

    let rate = frankfurter_rate(code).await?;

    Ok(TuiOutput::Json(json!({
        "success": true,
        "currency": code,
        "rate": rate,
    })))
}

fn is_valid_iso4217(code: &str) -> bool {
    code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase())
}

async fn frankfurter_rate(code: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let rate = reqwest::get("https://api.frankfurter.dev/latest?from=EUR&to=code")
        .await?
        .json::<serde_json::Value>()
        .await?
        .get("rates")
        .and_then(|r| r.get(code))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    Ok(rate)
}

fn handle_model_alias(from: &str, to: &str) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    let built_in = PricingEngine::built_in_aliases();
    let user = CodeBurnConfig::load(&std::env::current_dir()?)?.model_aliases;

    if !built_in.contains_key(from) && !user.contains_key(from) {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("alias not found: {from}"),
        )));
    }

    Ok(TuiOutput::Json(json!({
        "success": true,
        "alias": {
            "from": from,
            "to": to,
        },
    })))
}

fn handle_plan(action: &PlanAction) -> Result<TuiOutput, Box<dyn std::error::Error>> {
    let project_root = std::env::current_dir()?;
    let config = CodeBurnConfig::load(&project_root)?;

    match action {
        PlanAction::Set { name } => Ok(TuiOutput::Json(json!({
            "success": true,
            "plan": {
                "set": name,
                "usage": config.plan_usage(name)?,
            },
        }))),
        PlanAction::Reset => Ok(TuiOutput::Json(json!({
            "success": true,
            "plan": null,
        }))),
        PlanAction::Show => Ok(TuiOutput::Json(json!({
            "success": true,
            "plan": config.plan,
            "usage": config.plan_usage("unknown")?,
        }))),
    }
}

fn error_response(message: &str, show_usage: bool) -> serde_json::Value {
    let mut response = json!({
        "success": false,
        "error": message,
    });

    if show_usage {
        response["usage"] = json!(usage_lines());
    }

    response
}

fn usage_response(show_usage: bool) -> serde_json::Value {
    if show_usage {
        json!({
            "success": true,
            "error": null,
            "usage": usage_lines(),
        })
    } else {
        json!({
            "success": true,
            "error": null,
        })
    }
}

fn optimize_engine(
    _classified: &[codeburn_provider::SessionData],
) -> Result<Vec<codeburn_provider::SessionData>, Box<dyn std::error::Error>> {
    Ok(Vec::new())
}

fn usage_lines() -> &'static [&'static str] {
    &[
        "codeburn report",
        "codeburn report --period <period>",
        "codeburn report --from/--to",
        "codeburn report --provider <p>",
        "codeburn report --format json",
        "codeburn status",
        "codeburn export",
        "codeburn optimize",
        "codeburn compare",
        "codeburn currency [code]",
        "codeburn model-alias [from] [to]",
        "codeburn plan set/reset/show",
    ]
}
