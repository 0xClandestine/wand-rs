# 🪄 wand-rs

An expandable command-line interface that contains useful Foundry/Solidity related tools, built with Rust.

## Overview

`wand-rs` currently contains a single tool, but is designed as an extensible framework for Solidity development utilities. The CLI is built to grow over time with additional commands that solve specific pain points in smart contract development. While it only offers the `vacuum` tool at present, the goal is to expand this collection with more powerful utilities as development continues.

## Installation

### Prerequisites

- Rust and Cargo (install via [rustup](https://rustup.rs/))

### Building from source

```bash
git clone https://github.com/0xclandestine/wand-rs.git
cd wand-rs
cargo install --path .
```

## Available Tools

### Vacuum

The `vacuum` command helps you identify and optionally remove unused functions in your Solidity contracts.

#### Usage

```bash
# Find unused functions in a specific file
wand vacuum path/to/Contract.sol --root path/to/project

# Find unused functions in all Solidity files in a directory
wand vacuum path/to/contracts --root path/to/project

# Find and delete unused functions
wand vacuum path/to/Contract.sol --root path/to/project --delete

# Ignore specific function patterns (default ignores test functions)
wand vacuum path/to/Contract.sol --ignore "^test" --ignore "^_" --root path/to/project
```

#### Options

- `PATH`: Path to a Solidity file or directory to analyze
- `--root`: Root directory to search for function occurrences (default: current directory)
- `--delete`: Remove unused functions from the Solidity file(s)
- `--ignore`: Patterns for function names to ignore (default: `^test`)

## Adding New Commands

`wand-rs` is designed to be extensible. To add a new command:

1. Create a new module in the `src/commands` directory
2. Implement the command's functionality
3. Add the command to the `Commands` enum in `src/main.rs`
4. Update the match statement in the `main` function

## Contributing

Contributions are welcome!

<!-- ## License

[LICENSE INFO] -->

## Acknowledgements

- [Foundry](https://github.com/foundry-rs/foundry) - The inspiration for many of the utilities
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- Other Rust crates that make this project possible