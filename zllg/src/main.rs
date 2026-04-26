use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = zllg::cli::Cli::parse();
    cli.run()
}
