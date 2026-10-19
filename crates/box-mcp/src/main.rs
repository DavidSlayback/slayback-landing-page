//! `box-mcp [--seed N] [--frame-skip N] [--max-steps N] [--trace FILE]`: the box's MCP server on stdio. With
//! `--trace`, the session's record (trace, outcome and calls) is rewritten after every message, so a session cut off
//! mid-run still leaves it.

use std::io::{BufRead, Write};

use box_env::EnvConfig;
use box_mcp::Server;
use serde_json::{Value, json};

fn main() {
    let mut config = EnvConfig::default();
    let mut trace: Option<std::path::PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| fail(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--seed" => config.seed = number(&value()),
            "--frame-skip" => config.frame_skip = number(&value()),
            "--max-steps" => config.max_steps = number(&value()),
            "--trace" => trace = Some(value().into()),
            _ => fail(&format!("unknown argument {flag}")),
        }
    }

    // Bevy logs to stdout by default only with its log plugin, which the headless game doesn't add; stdout carries
    // nothing but protocol messages.
    let mut server = Server::new(config);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => server.handle(&msg),
            Err(e) => {
                Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }))
            }
        };
        if let Some(reply) = reply {
            writeln!(out, "{reply}").and_then(|_| out.flush()).expect("stdout");
        }
        if let Some(path) = &trace {
            let record = serde_json::to_string_pretty(&server.record()).expect("a record serialises");
            std::fs::write(path, record).unwrap_or_else(|e| fail(&format!("writing {}: {e}", path.display())));
        }
    }
}

fn number<T: std::str::FromStr>(s: &str) -> T {
    s.parse().unwrap_or_else(|_| fail(&format!("not a number: {s}")))
}

fn fail(message: &str) -> ! {
    eprintln!("box-mcp: {message}");
    std::process::exit(2)
}
