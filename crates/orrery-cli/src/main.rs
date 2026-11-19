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
use crate::commands::{CliContext, dispatch, exit_code_for, render_error};

fn trim_line_end_in_place(mut s: String) -> String {
    let len = s.trim_end_matches(['\r', '\n']).len();
    s.truncate(len);
    s
}

fn read_secret_from_stdin() -> orrery_core::Result<String> {
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|source| Error::Io {
            path: PathBuf::from("<stdin>"),
            source,
        })?;
    Ok(trim_line_end_in_place(line))
}

fn read_secret_input() -> orrery_core::Result<String> {
    if std::io::stdin().is_terminal() {
        let value = rpassword::prompt_password("Secret: ").map_err(|source| Error::Io {
            path: PathBuf::from("<tty>"),
            source,
        })?;
        Ok(trim_line_end_in_place(value))
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
            eprintln!("{}", render_error(&err));
            return ExitCode::from(exit_code_for(&err));
        }
    };

    let mut ctx = CliContext {
        out: Box::new(std::io::stdout()),
        err: Box::new(std::io::stderr()),
        secrets,
        read_secret: Box::new(read_secret_input),
        make_backends: Box::new(Backends::real),
        config_home: None,
    };

    match dispatch(command, &project, &mut ctx).await {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            let _ = writeln!(ctx.err, "{}", render_error(&err));
            ExitCode::from(exit_code_for(&err))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_line_end_keeps_allocation() {
        let mut s = String::with_capacity(32);
        s.push_str("tok\r\n");
        let ptr = s.as_ptr();
        let out = trim_line_end_in_place(s);
        assert_eq!(out, "tok");
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(trim_line_end_in_place("plain".to_string()), "plain");
    }
}
