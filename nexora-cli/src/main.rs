//! Thin offline CLI over `nexora-core`. No logic lives here (§44).
//!
//! Commands: `search <text>`, `query <text>`, `rename <old> <new> [--apply]`,
//! `daily:path`. Deterministic stdout; errors go to stderr with exit code 1.

use nexora_core::{daily, rename, search, vault, NoteId};
use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let Err(message) = run(&args) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

const USAGE: &str = "usage: nexora-cli <search|query|rename|daily:path> [args]";

fn bad_args(detail: &str) -> String {
    format!("{detail}\n{USAGE}")
}

fn run(args: &[String]) -> Result<(), String> {
    let command = args.first().ok_or_else(|| bad_args("missing command"))?;
    match command.as_str() {
        "search" | "query" => {
            if args.len() != 2 {
                return Err(bad_args(&format!("{command} expects exactly one query text")));
            }
            let text = &args[1];
            let index = vault::build_index(&vault::default_vault_root())
                .map_err(|error| error.to_string())?;
            let results = if command == "search" {
                search::search(&index, text)
            } else {
                search::search_advanced(&index, text)
            };
            for result in results {
                println!("{}\t{}", result.id, result.title);
            }
            Ok(())
        }
        "rename" => {
            if args.len() < 3 || args.len() > 4 {
                return Err(bad_args("rename expects <old> <new> [--apply]"));
            }
            let apply = match args.get(3) {
                None => false,
                Some(flag) if flag == "--apply" => true,
                Some(other) => return Err(bad_args(&format!("unknown flag: {other}"))),
            };
            let old = &args[1];
            let new = &args[2];
            let root = vault::default_vault_root();
            let plan = rename::plan(&root, &NoteId::new(old), new)
                .map_err(|error| error.to_string())?;
            if apply {
                rename::apply(&root, &plan).map_err(|error| error.to_string())?;
                println!("renamed\t{}", plan.new);
            } else {
                println!("rename\t{old}\t{}", plan.new);
                for edit in &plan.edits {
                    println!("{}\t{}", edit.id, edit.replacements);
                }
            }
            Ok(())
        }
        "daily:path" => {
            if args.len() != 1 {
                return Err(bad_args("daily:path takes no arguments"));
            }
            let id = daily::today_id();
            let path = vault::note_path(&vault::default_vault_root(), &id)
                .map_err(|error| error.to_string())?;
            println!("{}", path.display());
            Ok(())
        }
        other => Err(bad_args(&format!("unknown command: {other}"))),
    }
}
