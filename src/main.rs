mod app;
mod bigtext;
mod history;
mod paths;
mod settings;
mod stats;
mod typing;
mod ui;
mod words;

use clap::{Parser, Subcommand};

use settings::Kind;

/// typemaxxing: a typing speed test for your terminal. Runs entirely offline.
#[derive(Parser)]
#[command(name = "tmx", bin_name = "tmx", version)]
struct Cli {
    /// Length of a timed test in seconds (default: your last choice, or 30)
    #[arg(short, long, conflicts_with = "words")]
    time: Option<u64>,

    /// Type a fixed number of words instead of racing the clock
    #[arg(short, long)]
    words: Option<usize>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show your past results and averages
    History {
        /// How many recent results to list
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Command::History { limit }) = cli.command {
        return history::print(limit);
    }

    // Start from the last-used settings; command-line flags override them.
    let mut settings = settings::load();
    if let Some(n) = cli.words {
        settings.kind = Kind::Words;
        settings.words = n.clamp(1, 1000);
    } else if let Some(t) = cli.time {
        settings.kind = Kind::Time;
        settings.time = t.clamp(1, 3600);
    }

    // ratatui::init enters raw mode + the alternate screen and installs a
    // panic hook that restores the terminal, so a crash never leaves it broken.
    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, settings);
    ratatui::restore();

    let completed = result?;
    if let Some(best) = completed.iter().max_by(|a, b| a.wpm.total_cmp(&b.wpm)) {
        println!(
            "{} test{} this session · best {:.0} wpm at {:.0}% accuracy",
            completed.len(),
            if completed.len() == 1 { "" } else { "s" },
            best.wpm,
            best.accuracy
        );
    }
    Ok(())
}
