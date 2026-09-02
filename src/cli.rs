//! The scripting front-end: plan a directory, print the plan, optionally apply it.
//!
//! The CLI plans and applies in one pass, so there is no window in which the
//! filesystem could drift between the two; verification is the interactive
//! interface's job.

use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

use crate::applying::{apply_operations, prepare_operations};
use crate::paths::{display_path, file_name};
use crate::planning::{plan_directory, PlanOptions, RenameOp, RenamePlan};
use crate::presentation::{match_badge, skip_label};

pub use crate::presentation::MatchLevel as Level;

#[derive(Parser, Debug)]
#[command(
    name = "beaver",
    version,
    about = "Rename subtitle files to match the video files beside them.",
    long_about = "Rename subtitle files to match the video files beside them.\n\n\
                  Run without a path to open the terminal interface."
)]
pub struct Cli {
    /// Folder to scan. Without one, the terminal interface opens.
    pub path: Option<PathBuf>,

    /// Open the terminal interface, with PATH filled in if given.
    #[arg(long)]
    pub tui: bool,

    /// Process subfolders as well.
    #[arg(short, long)]
    pub recursive: bool,

    /// Video extension to include (repeatable).
    #[arg(long = "video-ext", value_name = "EXT")]
    pub video_ext: Vec<String>,

    /// Subtitle extension to include (repeatable).
    #[arg(long = "sub-ext", value_name = "EXT")]
    pub sub_ext: Vec<String>,

    /// How eager fuzzy matching should be.
    #[arg(long, value_enum, default_value_t = Level::Balanced)]
    pub level: Level,

    /// Fuzzy match threshold between 0 and 1, overriding --level.
    #[arg(long, value_name = "SCORE")]
    pub min_score: Option<f64>,

    /// Skip any subtitle whose target name is already taken.
    #[arg(long, conflicts_with = "force")]
    pub strict: bool,

    /// Show the planned renames without changing anything. The default.
    #[arg(long)]
    pub dry_run: bool,

    /// Apply the planned renames.
    #[arg(long)]
    pub apply: bool,

    /// Do not prompt for confirmation.
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Allow overwriting existing files. Dangerous, and CLI-only.
    #[arg(long, conflicts_with = "strict")]
    pub force: bool,
}

impl Cli {
    fn plan_options(&self) -> PlanOptions {
        let defaults = PlanOptions::default();
        PlanOptions {
            recursive: self.recursive,
            strict: self.strict,
            overwrite_existing: self.force,
            min_score: self.min_score.unwrap_or_else(|| self.level.score()),
            video_exts: if self.video_ext.is_empty() {
                defaults.video_exts
            } else {
                self.video_ext.clone()
            },
            sub_exts: if self.sub_ext.is_empty() {
                defaults.sub_exts
            } else {
                self.sub_ext.clone()
            },
        }
    }
}

pub fn run(cli: Cli) -> ExitCode {
    if cli.tui || cli.path.is_none() {
        return match crate::tui::run(cli.path.as_deref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Error: {error}");
                ExitCode::FAILURE
            }
        };
    }

    let path = cli.path.clone().expect("checked above");
    if cli.apply && cli.dry_run {
        eprintln!("Error: use either --dry-run or --apply, not both.");
        return ExitCode::from(2);
    }
    if let Some(score) = cli.min_score {
        if !(0.0..=1.0).contains(&score) {
            eprintln!("Error: --min-score must be between 0 and 1.");
            return ExitCode::from(2);
        }
    }

    let plan = match plan_directory(&path, &cli.plan_options()) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("Error: {error}");
            return ExitCode::from(2);
        }
    };

    print!("{}", format_plan(&plan));
    if plan.video_count == 0 {
        eprintln!("No video files found.");
        return ExitCode::FAILURE;
    }
    if plan.subtitle_count == 0 {
        eprintln!("No subtitle files found.");
        return ExitCode::FAILURE;
    }
    if !cli.apply || plan.operations.is_empty() {
        return ExitCode::SUCCESS;
    }

    if !cli.yes && !confirm(plan.operations.len()) {
        println!("Aborted.");
        return ExitCode::FAILURE;
    }

    // Planning and applying happen back to back here, so re-verifying the
    // filesystem state would only repeat what was just read.
    let result = match apply_operations(&prepare_operations(&plan), &plan.root, cli.force, false) {
        Ok(result) => result,
        Err(changed) => {
            eprintln!("Error: files changed while planning: {changed}");
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Renamed {} of {}.",
        result.applied.len(),
        plan.operations.len()
    );
    if result.failed.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprintln!("Some renames failed:");
    for outcome in &result.failed {
        eprintln!(
            "- {} -> {}: {}",
            outcome.source,
            outcome.target,
            outcome.error.as_deref().unwrap_or("unknown error")
        );
    }
    ExitCode::FAILURE
}

/// Render a plan as text, grouped by the directory each rename happens in.
pub fn format_plan(plan: &RenamePlan) -> String {
    let mut output = String::new();
    let mut grouped: BTreeMap<&Path, Vec<&RenameOp>> = BTreeMap::new();
    for operation in &plan.operations {
        let directory = operation.source.parent().unwrap_or(&plan.root);
        grouped.entry(directory).or_default().push(operation);
    }

    if grouped.is_empty() {
        output.push_str("No renames planned.\n");
    }
    for (directory, operations) in grouped {
        output.push_str(&format!(
            "Directory: {}\n",
            relative_directory(directory, &plan.root)
        ));
        for operation in operations {
            output.push_str(&format!(
                "- {}  ->  {}  ({})\n",
                file_name(&operation.source),
                file_name(&operation.destination),
                match_badge(&operation.reason)
            ));
        }
    }

    if !plan.skipped.is_empty() {
        output.push_str(&format!("\nSkipped {}:\n", plan.skipped.len()));
        for skipped in &plan.skipped {
            output.push_str(&format!(
                "- {}: {}\n",
                display_path(&skipped.path, &plan.root),
                skip_label(&skipped.reason)
            ));
        }
    }
    output
}

fn relative_directory(directory: &Path, root: &Path) -> String {
    if directory == root {
        return ".".into();
    }
    display_path(directory, root)
}

fn confirm(count: usize) -> bool {
    if !io::stdin().is_terminal() {
        return false;
    }
    print!("Apply {count} renames? [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    let answer = answer.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::demo_plan;

    #[test]
    fn formats_a_plan_grouped_by_directory() {
        let output = format_plan(&demo_plan());
        assert!(output.starts_with("Directory: .\n"));
        assert!(output.contains("Nebula.Archive.S01E01.zh-en.srt  ->  Nebula.Archive.S01E01.2160p.WEB-DL.srt  (episode S01E01)"));
        assert!(output.contains("Skipped 1:"));
        assert!(output.contains("Unsorted.Bonus.Feature.srt: No matching video"));
    }

    #[test]
    fn says_so_when_there_is_nothing_to_do() {
        let plan = crate::planning::plan_virtual_files(&["only.mkv"], &PlanOptions::default());
        assert_eq!(format_plan(&plan), "No renames planned.\n");
    }

    #[test]
    fn a_level_maps_onto_a_threshold() {
        let cli = Cli::parse_from(["beaver", "/tmp", "--level", "cautious"]);
        assert_eq!(cli.plan_options().min_score, Level::Cautious.score());

        let cli = Cli::parse_from([
            "beaver",
            "/tmp",
            "--level",
            "cautious",
            "--min-score",
            "0.5",
        ]);
        assert_eq!(cli.plan_options().min_score, 0.5);
    }

    #[test]
    fn extensions_fall_back_to_the_defaults() {
        let cli = Cli::parse_from(["beaver", "/tmp", "--sub-ext", ".ass"]);
        let options = cli.plan_options();
        assert_eq!(options.sub_exts, [".ass"]);
        assert_eq!(
            options.video_exts.len(),
            crate::planning::VIDEO_EXTS_DEFAULT.len()
        );
    }
}
