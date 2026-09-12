//! Command-line client for Ondera. Every command comes from the shared registry, so the CLI can
//! do exactly what the interface and an agent can do, on the running app or on a file.

use ondera_engine::control;
use ondera_tools::{coerce, merge, Backend};
use serde_json::Value;
use std::{path::PathBuf, process::ExitCode};

const USAGE: &str = "ondera-cli — command-line client for Ondera

USAGE
  ondera-cli [OPTIONS] <command> [--param value | param=value ...]
  ondera-cli commands                list every command
  ondera-cli help <command>          show one command's parameters

OPTIONS
  --file <session.ondera>   edit the file in this process and save after each change;
                            session.new creates it
  --live                    require the running Ondera app (the default when --file is absent)
  --params <json>           parameters as one JSON object, merged with --param values
  --agent                   mark created clips and notes as agent-made in the interface
  --compact                 single-line JSON output

EXAMPLES
  ondera-cli session.info
  ondera-cli track.add --kind midi --name Bass --instrument \"Sub Bass 808\"
  ondera-cli clip.create --trackId track-1 --startBar 0 --lengthBars 2 \\
      --notes '[{\"start\":0,\"length\":1,\"pitch\":36},{\"start\":2,\"length\":1,\"pitch\":43}]'
  ondera-cli --file song.ondera session.new
  ondera-cli --file song.ondera session.bounce --path mix.wav

Bars and beats are zero-based; note times are beats relative to their clip.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(message)) => {
            eprintln!("{message}\n\nRun `ondera-cli --help` for usage.");
            ExitCode::from(2)
        }
        Err(Failure::Command(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

enum Failure {
    Usage(String),
    Command(String),
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Command(message)
    }
}

fn run(args: &[String]) -> Result<(), Failure> {
    let mut file: Option<PathBuf> = None;
    let mut live = false;
    let mut params_json: Option<String> = None;
    let mut agent = false;
    let mut compact = false;
    let mut command: Option<String> = None;
    let mut pairs: Vec<(String, String)> = vec![];
    let mut i = 0;
    let next = |i: &mut usize, flag: &str| -> Result<String, Failure> {
        *i += 1;
        args.get(*i)
            .cloned()
            .ok_or_else(|| Failure::Usage(format!("{flag} needs a value")))
    };
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--file" | "-f" => file = Some(PathBuf::from(next(&mut i, arg)?)),
            "--live" => live = true,
            "--params" | "-p" => params_json = Some(next(&mut i, arg)?),
            "--agent" => agent = true,
            "--compact" | "-c" => compact = true,
            "--version" | "-V" => {
                println!("ondera-cli {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ if command.is_none() => {
                if arg.starts_with('-') {
                    return Err(Failure::Usage(format!("Unknown option `{arg}`")));
                }
                command = Some(arg.clone());
            }
            _ if command.as_deref() == Some("help") => pairs.push((arg.clone(), String::new())),
            _ => {
                if let Some(key) = arg.strip_prefix("--") {
                    if let Some((k, v)) = key.split_once('=') {
                        pairs.push((k.into(), v.into()));
                    } else {
                        pairs.push((key.into(), next(&mut i, arg)?));
                    }
                } else if let Some((k, v)) = arg.split_once('=') {
                    pairs.push((k.into(), v.into()));
                } else {
                    return Err(Failure::Usage(format!(
                        "Expected `--name value` or `name=value`, got `{arg}`"
                    )));
                }
            }
        }
        i += 1;
    }
    let Some(command) = command else {
        return Err(Failure::Usage("Missing command".into()));
    };
    if command == "commands" {
        for spec in control::COMMANDS {
            let params: Vec<String> = spec
                .params
                .iter()
                .map(|p| {
                    if p.required {
                        format!("--{}", p.name)
                    } else {
                        format!("[--{}]", p.name)
                    }
                })
                .collect();
            println!("{:<28} {}", spec.name, params.join(" "));
            println!("{:<28} {}", "", spec.doc);
        }
        return Ok(());
    }
    if command == "help" {
        let Some((name, _)) = pairs.first() else {
            println!("{USAGE}");
            return Ok(());
        };
        return describe(name);
    }
    if control::spec(&command).is_none() {
        // Let the registry produce its suggestion text.
        let mut backend = Backend::headless(None, false)?;
        backend.call(&command, &Value::Null, false)?;
        return Ok(());
    }
    let mut params: Vec<(String, Value)> = vec![];
    for (k, v) in &pairs {
        params.push((k.clone(), coerce(&command, k, v)?));
    }
    let params = merge(params_json.as_deref(), &params)?;

    let mut backend = match (&file, live) {
        (Some(_), true) => {
            return Err(Failure::Usage("--file and --live are exclusive".into()));
        }
        (Some(path), false) => Backend::headless(Some(path), command == "session.new")?,
        (None, _) => Backend::live().map_err(|e| {
            Failure::Command(format!(
                "{e}\nStart the Ondera app for live control, or pass --file <session.ondera> to edit a file."
            ))
        })?,
    };
    let result = backend.call(&command, &params, agent)?;
    if let Some(path) = backend.autosave()? {
        eprintln!("saved {}", path.display());
    }
    let text = if compact {
        serde_json::to_string(&result)
    } else {
        serde_json::to_string_pretty(&result)
    }
    .map_err(|e| e.to_string())?;
    println!("{text}");
    Ok(())
}

fn describe(name: &str) -> Result<(), Failure> {
    let Some(spec) = control::spec(name) else {
        return Err(Failure::Command(format!(
            "Unknown command `{name}`. Run `ondera-cli commands`."
        )));
    };
    println!("{}\n  {}\n", spec.name, spec.doc);
    if spec.params.is_empty() {
        println!("  No parameters.");
    }
    for p in spec.params {
        println!(
            "  --{:<16} {:<8} {}{}",
            p.name,
            p.kind.schema_type(),
            if p.required { "" } else { "(optional) " },
            p.doc
        );
    }
    Ok(())
}
