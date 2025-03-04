use anyhow::{anyhow, Result};
use clap::Parser;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
pub struct VacuumArgs {
    /// Path to a specific Solidity file to analyze.
    #[arg(long)]
    file: Option<PathBuf>,

    /// Directory containing Solidity files to analyze.
    #[arg(long)]
    dir: Option<PathBuf>,

    /// Root directory to search for function occurrences.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Remove unused functions from the Solidity file(s).
    #[arg(long)]
    delete: bool,

    /// Patterns for function names to ignore (e.g., '^test' for functions starting with 'test').
    #[arg(long, default_values = ["^test"])]
    ignore: Vec<String>,
}

pub fn run(args: VacuumArgs) -> Result<()> {
    if args.file.is_none() && args.dir.is_none() {
        return Err(anyhow!("Either --file or --dir must be specified."));
    }

    let mut total_unused = 0;

    if let Some(file) = &args.file {
        if !file.extension().map_or(false, |ext| ext == "sol") {
            println!("Warning: {:?} does not have a .sol extension.", file);
        }
        total_unused += process_single_file(file, &args.root, args.delete, &args.ignore)?;
    }

    if let Some(dir) = &args.dir {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "sol") {
                total_unused +=
                    process_single_file(&path.into(), &args.root, args.delete, &args.ignore)?;
            }
        }
    }

    println!("\nTotal unused functions found: {}", total_unused);
    Ok(())
}

fn extract_functions(sol_file: &PathBuf) -> Result<Vec<String>> {
    let content = fs::read_to_string(sol_file)?;
    let function_pattern = Regex::new(r"\bfunction\s+([a-zA-Z0-9_]+)\s*\(")?;

    Ok(function_pattern
        .captures_iter(&content)
        .map(|cap| cap[1].to_string())
        .collect())
}

fn count_function_occurrences(
    root_dir: &PathBuf,
    function_names: &[String],
) -> Result<HashMap<String, usize>> {
    let mut function_counts: HashMap<String, usize> =
        function_names.iter().map(|f| (f.clone(), 0)).collect();

    for entry in WalkDir::new(root_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "sol") {
            let content = fs::read_to_string(path)?;

            for func in function_names {
                *function_counts.entry(func.clone()).or_default() += content.matches(func).count();
            }
        }
    }

    Ok(function_counts)
}

fn should_ignore_function(func_name: &str, ignore_patterns: &[String]) -> bool {
    ignore_patterns
        .iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .any(|regex| regex.is_match(func_name))
}

fn remove_unused_functions(sol_file: &PathBuf, unused_functions: &[String]) -> Result<()> {
    let mut content = fs::read_to_string(sol_file)?;

    for func_name in unused_functions {
        let escaped_name = regex::escape(func_name);
        let pattern = format!(r"\bfunction\s+{}\s*\(", escaped_name);
        let function_pattern = Regex::new(&pattern)?;

        if let Some(mat) = function_pattern.find(&content) {
            let start_pos = mat.start();

            // Find the opening bracket after the function declaration
            let mut pos = mat.end();
            while pos < content.len() && content.chars().nth(pos) != Some('{') {
                pos += 1;
            }

            if pos < content.len() {
                let mut bracket_count = 1;
                pos += 1;

                // Count brackets to find the end of the function
                while bracket_count > 0 && pos < content.len() {
                    match content.chars().nth(pos) {
                        Some('{') => bracket_count += 1,
                        Some('}') => bracket_count -= 1,
                        _ => {}
                    }
                    pos += 1;
                }

                if bracket_count == 0 {
                    // Found the end of the function
                    let end_pos = pos;

                    // Look for NatSpec comments before the function
                    let mut natspec_start = start_pos;
                    if let Some(possible_natspec_start) = content[..start_pos].rfind("/**") {
                        let between_text = content[possible_natspec_start..start_pos].trim();
                        if between_text.starts_with("/**") && between_text.ends_with("*/") {
                            natspec_start = possible_natspec_start;
                        }
                    }

                    // Find the start of the line containing the natspec or function
                    let line_start = content[..natspec_start]
                        .rfind('\n')
                        .map_or(0, |pos| pos + 1);

                    // Remove the function and its natspec completely
                    let mut new_content = content[..line_start].to_string();
                    new_content.push_str(
                        &content[if end_pos < content.len()
                            && content.chars().nth(end_pos) == Some('\n')
                        {
                            end_pos + 1
                        } else {
                            end_pos
                        }..],
                    );

                    content = new_content;
                    println!("Removed function: {}", func_name);
                }
            }
        }
    }

    fs::write(sol_file, content)?;
    println!("Updated {:?} with unused functions removed.", sol_file);

    Ok(())
}

fn process_single_file(
    sol_file: &PathBuf,
    root_dir: &PathBuf,
    delete: bool,
    ignore_patterns: &[String],
) -> Result<usize> {
    let functions = extract_functions(sol_file)?;
    let function_counts = count_function_occurrences(root_dir, &functions)?;

    println!("\nFunction Usage Report for {:?}:", sol_file);
    let unused_functions: Vec<_> = functions
        .iter()
        .filter(|func| {
            let count = function_counts.get(*func).unwrap_or(&0);
            *count <= 1 && !should_ignore_function(func, ignore_patterns)
        })
        .cloned()
        .collect();

    for func in &functions {
        let count = function_counts.get(func).unwrap_or(&0);
        let ignored_msg = if should_ignore_function(func, ignore_patterns) {
            " (ignored)"
        } else {
            ""
        };
        println!("{}: {} occurrences{}", func, count, ignored_msg);
    }

    if !unused_functions.is_empty() {
        println!("\nFunctions marked for removal in {:?}:", sol_file);
        for func in &unused_functions {
            println!("- {}", func);
        }

        if delete {
            remove_unused_functions(sol_file, &unused_functions)?;
        }
    } else {
        println!("\nNo unused functions found in {:?}.", sol_file);
    }

    Ok(unused_functions.len())
}
