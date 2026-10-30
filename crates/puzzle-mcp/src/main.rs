//! `puzzle-mcp [--world NAME] [--tier N] [--seed N] [--max-calls N] [--json] [--trace FILE]`: one level of *Out of the
//! Box* as an MCP server on stdio. The level is the bench's for that world (default classic), tier (default 1) and seed
//! (default 1). `--max-calls` caps the tool calls (default 30, P21). `--json` adds a JSON observation to each reply.
//! With `--trace`, the session's record is rewritten after every message, so a session cut off mid-run still leaves it.

use mcp_stdio::fail;
use puzzle_core::World;
use puzzle_mcp::{MAX_CALLS, Pick, Server};

fn main() {
    let mut pick = Pick { world: World::Classic, tier: 1, seed: 1 };
    let (mut max_calls, mut json, mut trace) = (MAX_CALLS, false, None::<std::path::PathBuf>);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| fail("puzzle-mcp", &format!("{flag} needs a value")));
        match flag.as_str() {
            "--world" => {
                let name = value();
                pick.world = World::from_name(&name).unwrap_or_else(|| fail("puzzle-mcp", &format!("no world {name}")));
            }
            "--tier" => pick.tier = number(&value()),
            "--seed" => pick.seed = number(&value()),
            "--max-calls" => max_calls = number(&value()),
            "--json" => json = true,
            "--trace" => trace = Some(value().into()),
            _ => fail("puzzle-mcp", &format!("unknown argument {flag}")),
        }
    }
    let mut server = Server::new(pick, max_calls, json)
        .unwrap_or_else(|| fail("puzzle-mcp", &format!("no level for {pick:?} (tiers are 1 to 4)")));
    server.serve(trace.as_deref());
}

fn number<T: std::str::FromStr>(s: &str) -> T {
    s.parse().unwrap_or_else(|_| fail("puzzle-mcp", &format!("not a number: {s}")))
}
