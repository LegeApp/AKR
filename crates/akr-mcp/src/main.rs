//! The `akr-mcp` binary: a JSON-RPC server on stdio, bound to one workspace.
//!
//! ```text
//! akr-mcp [--dir <path>]
//! ```
//!
//! No network, no daemon, no state outside the workspace — the same three constraints the
//! `akr` binary lives under (`docs/07-cli.md` §1), because this is the same tool with a
//! different mouth.

use akr_mcp::{Server, serve};
use std::io::{BufReader, stdin, stdout};
use std::path::PathBuf;

fn main() -> std::process::ExitCode {
    let mut root = PathBuf::from(".");
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--dir" => match args.next() {
                Some(value) => root = PathBuf::from(value),
                None => {
                    eprintln!("error[AKR-C003]: --dir requires a value");
                    return std::process::ExitCode::from(2);
                }
            },
            "--version" | "-V" => {
                println!("akr-mcp {}", akr_mcp::protocol::SERVER_VERSION);
                return std::process::ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!(
                    "akr-mcp — the AKR knowledge tools over MCP\n\n\
                     USAGE\n    akr-mcp [--dir <path>]\n\n\
                     Speaks JSON-RPC 2.0 over stdio, one document per line. The MCP tools \
                     of docs/08-mcp.md §2 are listed by `tools/list`; every one of them \
                     calls the function `akr` calls.\n"
                );
                return std::process::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("error[AKR-C002]: unknown flag {other:?}");
                return std::process::ExitCode::from(2);
            }
        }
    }

    let server = Server::new(root);
    match serve(&server, BufReader::new(stdin()), stdout()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error[AKR-C042]: {error}");
            std::process::ExitCode::from(3)
        }
    }
}
