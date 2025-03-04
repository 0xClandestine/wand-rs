use clap::Parser;
use rayon::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

#[derive(Parser, Debug)]
pub struct VacuumArgs {
    /// Path to a Solidity file or directory to analyze.
    #[arg(value_name = "PATH")]
    path: PathBuf,

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
    let mut total_unused = 0;

    if args.path.is_file() {
        if !args.path.extension().map_or(false, |ext| ext == "sol") {
            println!("Warning: {:?} does not have a .sol extension.", args.path);
        }
        total_unused += process_single_file(&args.path, &args.root, args.delete, &args.ignore)?;
    } else if args.path.is_dir() {
        let sol_files = collect_sol_files(&args.path)?;
        total_unused += sol_files
            .par_iter()
            .map(|path| process_single_file(path, &args.root, args.delete, &args.ignore))
            .collect::<Result<Vec<usize>>>()?
            .iter()
            .sum::<usize>();
    } else {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Path {:?} does not exist.", args.path),
        ));
    }

    println!("\nTotal unused functions found: {}", total_unused);
    Ok(())
}

fn collect_sol_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut sol_files = Vec::new();
    let mut dirs_to_visit = vec![dir.clone()];

    while let Some(current_dir) = dirs_to_visit.pop() {
        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                dirs_to_visit.push(path);
            } else if path.extension().map_or(false, |ext| ext == "sol") {
                sol_files.push(path);
            }
        }
    }

    Ok(sol_files)
}

fn extract_functions(sol_file: &PathBuf) -> Result<Vec<String>> {
    let content = fs::read_to_string(sol_file)?;
    let function_pattern = Regex::new(r"\bfunction\s+([a-zA-Z0-9_]+)\s*\(")
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

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

    let sol_files = collect_sol_files(root_dir)?;

    let counts: Vec<HashMap<String, usize>> = sol_files
        .par_iter()
        .map(|path| {
            let content = fs::read_to_string(path).unwrap_or_default();
            let mut local_counts = HashMap::new();
            for func in function_names {
                local_counts.insert(func.clone(), content.matches(func).count());
            }
            local_counts
        })
        .collect();

    for count in counts {
        for (func, count) in count {
            *function_counts.entry(func).or_default() += count;
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
        let function_pattern =
            Regex::new(&pattern).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

        if let Some(mat) = function_pattern.find(&content) {
            let start_pos = mat.start();

            // Find the opening bracket after the function declaration
            let mut pos = mat.end();
            let mut found_semicolon = false;
            while pos < content.len() {
                match content.chars().nth(pos) {
                    Some('{') => break,
                    Some(';') => {
                        found_semicolon = true;
                        break;
                    }
                    _ => {}
                }
                pos += 1;
            }

            if pos < content.len() {
                let end_pos = if found_semicolon {
                    pos + 1
                } else {
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
                        pos
                    } else {
                        continue;
                    }
                };

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

                // Find the end of the line after the function
                let next_line_start = content[end_pos..]
                    .find('\n')
                    .map(|pos| end_pos + pos + 1)
                    .unwrap_or(content.len());

                // Remove the function and its natspec completely
                let mut new_content = String::new();
                new_content.push_str(&content[..line_start]);
                if next_line_start < content.len() {
                    new_content.push_str(&content[next_line_start..]);
                }
                content = new_content;
                println!("Removed function: {}", func_name);
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
        if !should_ignore_function(func, ignore_patterns) {
            let count = function_counts.get(func).unwrap_or(&0);
            let color = match count {
                1 => RED,
                2 => YELLOW,
                _ => GREEN,
            };
            println!("{}{}{}: {}", color, func, RESET, count);
        }
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
