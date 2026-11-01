// Written by Kone Tshivhinda & Claude
//! `cargo run -p timelex-new-service -- <name> [--root <path>]`

use std::path::{Path, PathBuf};

use timelex_new_service::generate;

const USAGE: &str = "usage: timelex-new-service <name> [--root <path>]\n\n\
     <name> is 2 to 31 characters: a lowercase letter, then lowercase letters, digits or\n\
     underscores. It becomes the schema, the service role and the directory name.";

fn main() {
    let mut name: Option<String> = None;
    let mut root = PathBuf::from(".");
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            "--root" => match args.next() {
                Some(path) => root = PathBuf::from(path),
                None => fail("--root needs a path"),
            },
            other if other.starts_with('-') => fail(&format!("unknown option {other:?}")),
            _ if name.is_some() => fail("give one service name"),
            other => name = Some(other.to_owned()),
        }
    }
    let Some(name) = name else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };

    match generate(&root, &name) {
        Ok(report) => {
            for path in &report.created {
                println!("created {}", display(&root, path));
            }
            for path in &report.patched {
                println!("patched {}", display(&root, path));
            }
            println!(
                "\nNext: give the service a contract under contracts/openapi, replace the items\n\
                 example, and route the new operations from the gateway."
            );
        }
        Err(error) => fail(&error.to_string()),
    }
}

fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}
