mod args;
mod commands;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use orrery_core::Error;
use orrery_core::engine::Backends;
use orrery_core::security::OsKeyringStore;

use crate::args::Cli;
use crate::commands::{CliContext, dispatch, exit_code_for};

fn read_secret_from_stdin() -> orrery_core::Result<String> {
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|source| Error::Io {
            path: PathBuf::from("<stdin>"),
            source,
        })?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

fn read_secret_input() -> orrery_core::Result<String> {
    if std::io::stdin().is_terminal() {
        let value = rpassword::prompt_password("Secret: ").map_err(|source| Error::Io {
            path: PathBuf::from("<tty>"),
            source,
        })?;
        Ok(value.trim_end_matches(['\r', '\n']).to_string())
    } else {
        read_secret_from_stdin()
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = cli.validate() {
        e.exit();
    }

    let project = cli.project.clone();
    let command = cli.into_command();

    let secrets = match OsKeyringStore::new() {
        Ok(store) => Arc::new(store),
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };

    let mut ctx = CliContext {
        out: Box::new(std::io::stdout()),
        err: Box::new(std::io::stderr()),
        secrets,
        read_secret: Box::new(read_secret_input),
        make_backends: Box::new(Backends::real),
    };

    match dispatch(command, &project, &mut ctx).await {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            let _ = writeln!(ctx.err, "{err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}
