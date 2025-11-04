use clap::{CommandFactory, Parser};
use clap_complete::Shell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use yars_yaml_formatter::format_yaml_string;

#[derive(Parser, Debug)]
#[command(
    name = "yars-format",
    version,
    about = "Format YAML files using the yars formatter",
    long_about = None
)]
struct Cli {
    /// Run in check mode (report changes without writing)
    #[arg(long)]
    check: bool,

    /// Verbose output (list each processed file)
    #[arg(short, long)]
    verbose: bool,

    /// Generate shell completion script for the given shell
    #[arg(long = "generate-completions", value_enum)]
    generate_completions: Option<Shell>,

    /// YAML files to format
    #[arg(
        value_name = "FILE",
        required_unless_present = "generate_completions",
        num_args = 1..
    )]
    files: Vec<PathBuf>,
}

struct FileOutcome {
    changed: bool,
    lines_changed: usize,
}

pub fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(shell) = cli.generate_completions {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        clap_complete::generate(shell, &mut cmd, bin_name, &mut io::stdout());
        return ExitCode::SUCCESS;
    }

    let mut changed_count = 0usize;
    let mut error_count = 0usize;
    let mut success_count = 0usize;

    for path in &cli.files {
        match process_file(path, cli.check) {
            Ok(outcome) => {
                success_count += 1;
                if outcome.changed {
                    changed_count += 1;
                    if cli.verbose {
                        if cli.check {
                            println!(
                                "{} - would reformat ({} differing line(s))",
                                path.display(),
                                outcome.lines_changed
                            );
                        } else {
                            println!(
                                "{} - reformatted ({} line(s) changed)",
                                path.display(),
                                outcome.lines_changed
                            );
                        }
                    }
                } else if cli.verbose {
                    println!("{} - already formatted", path.display());
                }
            }
            Err(err) => {
                error_count += 1;
                eprintln!("Error: {}", err);
            }
        }
    }

    if cli.check {
        println!(
            "Checked {} file(s); {} would change.",
            success_count, changed_count
        );
    } else {
        println!(
            "Formatted {} file(s); {} updated, {} unchanged.",
            success_count,
            changed_count,
            success_count.saturating_sub(changed_count)
        );
    }

    if error_count > 0 {
        eprintln!("Encountered {} error(s).", error_count);
        return ExitCode::from(2);
    }

    if cli.check && changed_count > 0 {
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

fn process_file(path: &Path, check_only: bool) -> Result<FileOutcome, String> {
    let original = fs::read_to_string(path)
        .map_err(|err| format!("{}: Failed to read file: {}", path.display(), err))?;

    let formatted =
        format_yaml_string(&original).map_err(|err| format!("{}: {}", path.display(), err))?;

    let changed = original != formatted;
    if changed {
        if !check_only {
            fs::write(path, &formatted)
                .map_err(|err| format!("{}: Failed to write file: {}", path.display(), err))?;
        }
        let lines_changed = count_changed_lines(&original, &formatted);
        Ok(FileOutcome {
            changed: true,
            lines_changed,
        })
    } else {
        Ok(FileOutcome {
            changed: false,
            lines_changed: 0,
        })
    }
}

fn count_changed_lines(original: &str, formatted: &str) -> usize {
    let original_lines: Vec<&str> = original.lines().collect();
    let formatted_lines: Vec<&str> = formatted.lines().collect();
    let max_len = original_lines.len().max(formatted_lines.len());

    let mut diff = 0usize;
    for idx in 0..max_len {
        let orig_line = original_lines.get(idx).copied().unwrap_or("");
        let new_line = formatted_lines.get(idx).copied().unwrap_or("");
        if orig_line != new_line {
            diff += 1;
        }
    }

    diff
}
