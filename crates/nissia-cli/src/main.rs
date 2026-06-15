mod cmd;
mod validate;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "nissia",
    about = "AI agent-optimized browser CLI — snap + act",
    version,
    after_long_help = AGENT_GUIDE
)]
struct Cli {
    /// Chrome debugging port
    #[arg(long, default_value = "9222", global = true)]
    port: u16,

    /// Output format: text, json, or ndjson (auto-detects json when piped)
    #[arg(long, global = true)]
    output: Option<String>,

    /// Preview what would happen without executing
    #[arg(long, global = true)]
    dry_run: bool,

    /// Browser language for Accept-Language header (e.g. en-US, ko, ja)
    #[arg(long, default_value = "en-US", global = true)]
    lang: String,

    /// Override geolocation (e.g. "37.7749,-122.4194" for San Francisco)
    #[arg(long, global = true)]
    geo: Option<String>,

    /// Override JS navigator.language / Intl locale (e.g. "en-US")
    #[arg(long, global = true)]
    locale: Option<String>,

    /// Override User-Agent string
    #[arg(long, global = true)]
    user_agent: Option<String>,

    /// Verbose logging
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract interactable elements as @eN references. Always snap before acting.
    /// Output: @e1 [role] "label" attrs — one element per line
    Snap {
        /// URL to navigate to (optional if already on a page)
        url: Option<String>,

        /// CSS selector to limit extraction scope (e.g. "main", "#content")
        #[arg(long)]
        focus: Option<String>,
    },

    /// Read visible text content as structured markdown. Use when you need to understand
    /// page content (lists, tables, headings) — not just interactable elements.
    Read {
        /// URL to navigate to (optional if already on a page)
        url: Option<String>,

        /// CSS selector to limit reading scope (e.g. "main", "#content", ".pr-list")
        #[arg(long)]
        focus: Option<String>,

        /// Maximum number of lines to return [default: 200]
        #[arg(long, default_value = "200")]
        max_lines: usize,
    },

    /// Click an element by @eN reference from snap output.
    /// Returns updated page snapshot automatically (use --no-snap to disable).
    Click {
        /// Element reference from snap (e.g. @e1)
        #[arg(name = "ref")]
        element_ref: String,

        /// Skip automatic re-snap after action
        #[arg(long)]
        no_snap: bool,
    },

    /// Set an input field's value (clears existing). Use for <input>, <textarea>.
    /// Returns updated page snapshot automatically.
    Fill {
        /// Element reference from snap (e.g. @e2)
        #[arg(name = "ref")]
        element_ref: String,

        /// Value to set
        value: String,

        /// Skip automatic re-snap after action
        #[arg(long)]
        no_snap: bool,
    },

    /// Type text character by character with key events. Use for autocomplete/search.
    /// Returns updated page snapshot automatically.
    Type {
        /// Element reference from snap (e.g. @e2)
        #[arg(name = "ref")]
        element_ref: String,

        /// Text to type
        text: String,

        /// Skip automatic re-snap after action
        #[arg(long)]
        no_snap: bool,
    },

    /// Select an option in a <select> dropdown by value.
    /// Returns updated page snapshot automatically.
    Select {
        /// Element reference from snap (e.g. @e3)
        #[arg(name = "ref")]
        element_ref: String,

        /// Option value to select
        value: String,

        /// Skip automatic re-snap after action
        #[arg(long)]
        no_snap: bool,
    },

    /// Scroll the page in a direction.
    /// Returns updated page snapshot automatically.
    Scroll {
        /// Direction: up, down, left, right
        #[arg(default_value = "down")]
        direction: String,

        /// Pixels to scroll [default: 400]
        #[arg(long)]
        amount: Option<i64>,

        /// Skip automatic re-snap after action
        #[arg(long)]
        no_snap: bool,
    },

    /// Capture a PNG screenshot of the current page
    Screenshot {
        /// Output file path [default: stdout as base64 in JSON mode]
        #[arg(short, long = "file")]
        file: Option<String>,
    },

    /// Wait for navigation, a CSS selector to appear, or a timeout in ms
    Wait {
        /// "navigation" | CSS selector | milliseconds (e.g. "2000")
        condition: String,
    },

    /// Execute JavaScript on the current page and return the result.
    /// Use for extracting data that snap/read can't capture (e.g. product cards on Amazon).
    Eval {
        /// JavaScript expression to evaluate
        expression: String,
    },

    /// Dismiss cookie banners, consent popups and blocking overlays so the page is readable.
    Dismiss,

    /// Press a key: enter, tab, escape, backspace, arrowup/down/left/right, space, etc.
    /// Submit a search (enter), move between fields (tab), or pick an autocomplete
    /// suggestion (arrowdown then enter).
    Key {
        /// Key name (e.g. enter, tab, escape, arrowdown)
        key: String,
    },

    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Record a workflow
    Record {
        #[command(subcommand)]
        action: RecordAction,
    },

    /// Replay a recorded workflow
    Replay {
        /// Workflow name
        name: String,

        /// Speed multiplier (1.0 = original speed, 0 = instant)
        #[arg(long, default_value = "0")]
        speed: f64,
    },

    /// Launch Chrome with remote debugging enabled
    Browser {
        #[command(subcommand)]
        action: BrowserAction,
    },

    /// Start an MCP server exposing nissia tools over JSON-RPC (stdio)
    Mcp,

    /// (Optional turbo, opt-in) Autonomous agent: drives the browser with a cheap
    /// internal LLM and prints ONLY the final answer. Needs an API key
    /// (NISSIA_AGENT_API_KEY); the navigate/search/batch modes need no key.
    Agent {
        /// Goal in natural language
        goal: String,
        /// Optional starting URL
        #[arg(long)]
        url: Option<String>,
        /// Max reasoning steps
        #[arg(long)]
        max_steps: Option<usize>,
    },

    /// Run a sequence of steps from stdin on ONE connection (fast: a whole flow in
    /// one process / one round trip). One verb per line:
    /// goto/snap/read/eval/click/fill/type/select/scroll/wait.
    Batch,

    /// Cheap web search (no API key required by default); prints compact results.
    Search {
        /// Search query
        query: String,
        /// Number of results [default: 5]
        #[arg(long, default_value = "5")]
        n: usize,
        /// Also read the top result as focused markdown
        #[arg(long)]
        read: bool,
        /// Search inside the running browser (DuckDuckGo) instead of HTTP. Reliable,
        /// free, no rate limits. Best for Agente/Navegar (reuses the open window).
        #[arg(long)]
        browser: bool,
        /// Human navigation: after a --browser search, CLICK this result (1 = top) with a
        /// real mouse click, landing on the page with a natural referrer (not a teleport).
        #[arg(long)]
        open: Option<usize>,
    },

    /// Show JSON Schema for a command's inputs and outputs
    Schema {
        /// Command name (e.g. snap, click, fill). Omit for full schema.
        command: Option<String>,
    },

    /// Write AGENTS.md to the current directory for AI coding agent skill discovery.
    /// Run this once in your project to enable Claude Code, Codex, opencode, etc.
    Init,
}

#[derive(Subcommand)]
enum SessionAction {
    /// Save current browser session
    Save { name: String },
    /// Restore a saved session
    Load { name: String },
    /// List saved sessions
    List,
    /// Delete a saved session
    Delete { name: String },
}

#[derive(Subcommand)]
enum RecordAction {
    /// Start recording
    Start {
        /// Recording name (optional, auto-generated if not provided)
        name: Option<String>,
    },
    /// Stop recording and save
    Stop,
    /// List recorded workflows
    List,
    /// Delete a recorded workflow
    Delete { name: String },
}

#[derive(Subcommand)]
enum BrowserAction {
    /// Launch Chrome with remote debugging. Use --background for agent use
    Launch {
        /// Run in headless mode
        #[arg(long)]
        headless: bool,

        /// Run in background (detach immediately, use "browser stop" to terminate)
        #[arg(long)]
        background: bool,

        /// Browser profile name [default: "default"]. Persistent profiles keep
        /// cookies and login state between sessions, reducing bot detection.
        #[arg(long)]
        profile: Option<String>,

        /// Auto-stop Chrome after N minutes of inactivity (no nissia commands).
        /// Recommended for AI agent sessions to prevent orphaned Chrome processes.
        #[arg(long)]
        idle_timeout: Option<u32>,
    },
    /// Stop a running Chrome instance
    Stop,
    /// Check if Chrome is running
    Status,
}

const AGENT_GUIDE: &str = "\
WORKFLOW (for AI agents):
  1. nissia browser launch --background   # start Chrome (persistent profile)
  2. nissia snap <url>                    # page structure + elements + section summaries
  3. nissia click/fill/type @eN           # act (auto re-snap included in response)
  4. nissia browser stop                  # stop Chrome when done

  Actions (click, fill, type, select, scroll) automatically return a fresh
  page snapshot — no need to call snap manually after each action.

SNAP vs READ vs EVAL:
  snap   → page structure + interactable elements + section content summaries
  read   → full visible text as structured markdown
  eval   → execute custom JavaScript (for complex data extraction)
  All support --focus=<selector> to scope to a page section.

ELEMENT REFERENCES:
  snap output: @e1 [button] \"Sign In\" id=\"submit\" expanded desc=\"...\"
  use @e1 in: nissia click @e1 / nissia fill @e1 \"value\"
  refs persist on disk — valid until next snap

BROWSER ENVIRONMENT (for international sites):
  --locale=en-US    Override JS navigator.language (fixes currency: KRW→USD)
  --geo=LAT,LON     Override geolocation (e.g. 37.7749,-122.4194)
  --user-agent=UA   Override User-Agent string
  --lang=LANG       Set Accept-Language HTTP header [default: en-US]

  Example: nissia snap https://amazon.com --locale=en-US --geo=37.7,-122.4

KEY FLAGS:
  --no-snap          Skip auto re-snap after actions (for record/replay)
  --focus=SELECTOR   Limit scope to a CSS selector (snap/read)
  --profile=NAME     Named browser profile (browser launch)

EXAMPLES:
  nissia snap https://github.com/login        # form elements + section summaries
  nissia fill @e2 \"user\" && nissia click @e5  # fill + click (auto re-snap)
  nissia read --focus=\"main\"                  # read main section as markdown
  nissia eval \"document.title\"                # run JavaScript on page
  nissia eval \"JSON.stringify([...document.querySelectorAll('h2')].map(h=>h.textContent))\"
  nissia session save github                  # persist cookies/state
  nissia session load github                  # restore later
  nissia record start my-flow                 # record a workflow
  nissia replay my-flow                       # replay with zero LLM cost

SCHEMA / MCP / INIT:
  nissia schema [command]                     # JSON Schema introspection
  nissia mcp                                  # start MCP server (JSON-RPC stdio)
  nissia init                                 # create AGENTS.md for AI coding agents

SAFETY:
  --dry-run on any mutation shows what would execute without acting
  element refs are validated (rejects invalid formats)
  CSS selectors are validated (rejects javascript: protocol)";

/// Resolved output format after TTY detection.
fn resolve_output_format(explicit: Option<&str>) -> &str {
    match explicit {
        Some(fmt) => fmt,
        None => {
            // Default to text — contextual snap output is most useful for LLMs.
            // Use --output=json explicitly when you need structured JSON.
            "text"
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber_init();
    }

    let fmt = resolve_output_format(cli.output.as_deref()).to_owned();

    if let Err(e) = dispatch(cli, &fmt).await {
        emit_error(&e, &fmt);
        std::process::exit(1);
    }
}

fn emit_error(err: &anyhow::Error, fmt: &str) {
    let msg = err.to_string();
    let code = if msg.contains("Cannot connect")
        || msg.contains("BrowserNotFound")
        || msg.contains("Chrome")
    {
        "BROWSER_NOT_CONNECTED"
    } else if msg.contains("not found") || msg.contains("No such") || msg.contains("does not exist")
    {
        "NOT_FOUND"
    } else if msg.contains("Invalid") || msg.contains("invalid") || msg.contains("Unknown") {
        "INVALID_INPUT"
    } else if msg.contains("timeout") || msg.contains("Timeout") {
        "TIMEOUT"
    } else {
        "ERROR"
    };

    if fmt == "json" {
        eprintln!(
            "{}",
            serde_json::json!({"error": {"code": code, "message": msg}})
        );
    } else {
        eprintln!("error: {msg}");
    }
}

fn parse_geo(s: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() == 2 {
        let lat = parts[0].trim().parse::<f64>().ok()?;
        let lon = parts[1].trim().parse::<f64>().ok()?;
        Some((lat, lon))
    } else {
        None
    }
}

async fn dispatch(cli: Cli, fmt: &str) -> anyhow::Result<()> {
    let emu = nissia_core::snap::EmulationOptions {
        geo: cli.geo.as_deref().and_then(parse_geo),
        locale: cli.locale.clone(),
        user_agent: cli.user_agent.clone(),
    };

    match cli.command {
        Commands::Snap { url, focus } => {
            if let Some(f) = &focus {
                validate::css_selector(f)?;
            }
            cmd::snap::run(
                cli.port,
                url.as_deref(),
                focus.as_deref(),
                fmt,
                &cli.lang,
                &emu,
            )
            .await?;
        }
        Commands::Read {
            url,
            focus,
            max_lines,
        } => {
            if let Some(f) = &focus {
                validate::css_selector(f)?;
            }
            cmd::read::run(
                cli.port,
                url.as_deref(),
                focus.as_deref(),
                fmt,
                &cli.lang,
                max_lines,
                &emu,
            )
            .await?;
        }
        Commands::Click {
            element_ref,
            no_snap,
        } => {
            validate::element_ref(&element_ref)?;
            cmd::action::run_click(
                cli.port,
                &element_ref,
                fmt,
                cli.dry_run,
                no_snap,
                &cli.lang,
                &emu,
            )
            .await?;
        }
        Commands::Fill {
            element_ref,
            value,
            no_snap,
        } => {
            validate::element_ref(&element_ref)?;
            validate::fill_value(&value)?;
            cmd::action::run_fill(
                cli.port,
                &element_ref,
                &value,
                fmt,
                cli.dry_run,
                no_snap,
                &cli.lang,
                &emu,
            )
            .await?;
        }
        Commands::Type {
            element_ref,
            text,
            no_snap,
        } => {
            validate::element_ref(&element_ref)?;
            validate::fill_value(&text)?;
            cmd::action::run_type(
                cli.port,
                &element_ref,
                &text,
                fmt,
                cli.dry_run,
                no_snap,
                &cli.lang,
                &emu,
            )
            .await?;
        }
        Commands::Select {
            element_ref,
            value,
            no_snap,
        } => {
            validate::element_ref(&element_ref)?;
            cmd::action::run_select(
                cli.port,
                &element_ref,
                &value,
                fmt,
                cli.dry_run,
                no_snap,
                &cli.lang,
                &emu,
            )
            .await?;
        }
        Commands::Scroll {
            direction,
            amount,
            no_snap,
        } => {
            cmd::action::run_scroll(
                cli.port,
                &direction,
                amount,
                fmt,
                cli.dry_run,
                no_snap,
                &cli.lang,
                &emu,
            )
            .await?;
        }
        Commands::Screenshot { file } => {
            cmd::action::run_screenshot(cli.port, file.as_deref(), fmt).await?;
        }
        Commands::Wait { condition } => {
            if condition != "navigation" && condition.parse::<u64>().is_err() {
                validate::css_selector(&condition)?;
            }
            cmd::action::run_wait(cli.port, &condition, fmt).await?;
        }
        Commands::Eval { expression } => {
            cmd::action::run_eval(cli.port, &expression, fmt).await?;
        }
        Commands::Dismiss => {
            cmd::action::run_dismiss(cli.port, fmt).await?;
        }
        Commands::Key { key } => {
            cmd::action::run_key(cli.port, &key, fmt).await?;
        }
        Commands::Session { action } => match action {
            SessionAction::Save { name } => {
                cmd::session::run_save(cli.port, &name, fmt).await?;
            }
            SessionAction::Load { name } => {
                cmd::session::run_load(cli.port, &name, fmt).await?;
            }
            SessionAction::List => {
                cmd::session::run_list(fmt)?;
            }
            SessionAction::Delete { name } => {
                cmd::session::run_delete(&name, fmt)?;
            }
        },
        Commands::Record { action } => match action {
            RecordAction::Start { name } => {
                cmd::record::run_start(name.as_deref(), fmt)?;
            }
            RecordAction::Stop => {
                cmd::record::run_stop(fmt)?;
            }
            RecordAction::List => {
                cmd::record::run_list(fmt)?;
            }
            RecordAction::Delete { name } => {
                cmd::record::run_delete(&name, fmt)?;
            }
        },
        Commands::Replay { name, speed } => {
            cmd::replay::run(cli.port, &name, speed, fmt, cli.dry_run).await?;
        }
        Commands::Browser { action } => match action {
            BrowserAction::Launch {
                headless,
                background,
                profile,
                idle_timeout,
            } => {
                cmd::browser::run_launch(
                    cli.port,
                    headless,
                    background,
                    profile.as_deref(),
                    idle_timeout,
                    fmt,
                )?;
            }
            BrowserAction::Stop => {
                cmd::browser::run_stop(cli.port, fmt)?;
            }
            BrowserAction::Status => {
                cmd::browser::run_status(cli.port, fmt)?;
            }
        },
        Commands::Mcp => {
            cmd::mcp::run(cli.port).await?;
        }
        Commands::Agent {
            goal,
            url,
            max_steps,
        } => {
            cmd::agent::run(
                cli.port,
                &goal,
                url.as_deref(),
                max_steps,
                &cli.lang,
                &emu,
                cli.verbose,
            )
            .await?;
        }
        Commands::Batch => {
            cmd::batch::run(cli.port, &cli.lang, &emu).await?;
        }
        Commands::Search {
            query,
            n,
            read,
            browser,
            open,
        } => {
            cmd::search::run(cli.port, &query, n, read, browser, open, fmt, &cli.lang, &emu).await?;
        }
        Commands::Schema { command } => {
            cmd::schema::run(command.as_deref(), fmt);
        }
        Commands::Init => {
            let mut created = Vec::new();

            // Create .nissia/ directory for project-scope workflows
            let nissia_dir = std::path::Path::new(".nissia");
            if !nissia_dir.exists() {
                std::fs::create_dir_all(nissia_dir.join("workflows"))?;
                created.push(".nissia/workflows/");
            }

            // Create AGENTS.md for AI coding agent skill discovery
            let agent_path = std::path::Path::new("AGENTS.md");
            if !agent_path.exists() {
                let agent_md = include_str!("../../../AGENTS.md");
                std::fs::write(agent_path, agent_md)?;
                created.push("AGENTS.md");
            }

            // Create .nissia/.gitkeep so empty dir is tracked by git
            let gitkeep = nissia_dir.join("workflows").join(".gitkeep");
            if !gitkeep.exists() {
                std::fs::write(&gitkeep, "")?;
            }

            if created.is_empty() {
                println!("Already initialized.");
            } else {
                println!("Initialized nissia project:");
                for item in &created {
                    println!("  + {item}");
                }
                println!("\nWorkflows will be saved to .nissia/workflows/ (project scope).");
            }
        }
    }

    Ok(())
}

fn tracing_subscriber_init() {
    use tracing_subscriber::fmt;
    let _ = fmt::try_init();
}
