//! `box-mcp [--seed N] [--frame-skip N] [--max-steps N] [--rim M] [--max-calls N] [--trace FILE]`: the box's MCP
//! server on stdio. `--rim` sets the first box's rim (the benchmark's mini box). `--max-calls` caps the session's
//! tool calls (a benchmark's turn budget). With
//! `--trace`, the session's record (trace, outcome and calls) is rewritten after every message, so a session cut off
//! mid-run still leaves it.

use box_env::EnvConfig;
use box_mcp::Server;

fn main() {
    let mut config = EnvConfig::default();
    let mut trace: Option<std::path::PathBuf> = None;
    let mut max_calls: Option<u32> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| fail(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--seed" => config.seed = number(&value()),
            "--frame-skip" => config.frame_skip = number(&value()),
            "--max-steps" => config.max_steps = number(&value()),
            "--rim" => config.rim = Some(number(&value())),
            "--max-calls" => max_calls = Some(number(&value())),
            "--trace" => trace = Some(value().into()),
            _ => fail(&format!("unknown argument {flag}")),
        }
    }

    // Bevy logs to stdout by default only with its log plugin, which the headless game doesn't add; stdout carries
    // nothing but protocol messages.
    let mut server = Server::new(config);
    if let Some(max) = max_calls {
        server = server.with_max_calls(max);
    }
    server.serve(trace.as_deref());
}

fn number<T: std::str::FromStr>(s: &str) -> T {
    s.parse().unwrap_or_else(|_| fail(&format!("not a number: {s}")))
}

fn fail(message: &str) -> ! {
    mcp_stdio::fail("box-mcp", message)
}
