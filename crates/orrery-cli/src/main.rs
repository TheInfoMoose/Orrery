mod args;

use clap::Parser;

use crate::args::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.validate() {
        e.exit();
    }
    println!("{:?}", cli.into_command());
}
