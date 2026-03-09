use base64::{engine::general_purpose::STANDARD, Engine as _};
use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Parser, Debug)]
#[command(name = "bambuctl", about = "Generic CLI for the Bambu LAN Viewer HTTP API")]
struct Cli {
    /// Base URL for the Bambu LAN Viewer API origin.
    #[arg(long, env = "BAMBUCTL_BASE_URL", default_value = "http://127.0.0.1:8080")]
    base_url: String,

    /// Optional username for HTTP Basic Auth.
    #[arg(long, env = "BAMBUCTL_USERNAME")]
    username: Option<String>,

    /// Optional password for HTTP Basic Auth.
    #[arg(long, env = "BAMBUCTL_PASSWORD")]
    password: Option<String>,

    /// Emit compact JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Subcommand, Debug)]
enum RootCommand {
    Printers(PrintersCommand),
}

#[derive(Args, Debug)]
struct PrintersCommand {
    #[command(subcommand)]
    command: PrintersSubcommand,
}

#[derive(Subcommand, Debug)]
enum PrintersSubcommand {
    List,
    Status(IdArgs),
    Light(LightArgs),
    Pause(IdArgs),
    Resume(IdArgs),
    Stop(IdArgs),
    Home(IdArgs),
    Move(MoveArgs),
    NozzleTemp(TempArgs),
    BedTemp(TempArgs),
    Extrude(ExtrudeArgs),
}

#[derive(Args, Debug)]
struct IdArgs {
    #[arg(long)]
    id: i64,
}

#[derive(Args, Debug)]
struct LightArgs {
    #[arg(long)]
    id: i64,

    #[arg(long, conflicts_with = "off")]
    on: bool,

    #[arg(long, conflicts_with = "on")]
    off: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Axis {
    X,
    Y,
    Z,
}

#[derive(Args, Debug)]
struct MoveArgs {
    #[arg(long)]
    id: i64,

    #[arg(long, value_enum)]
    axis: Axis,

    #[arg(long)]
    distance: f64,

    #[arg(long)]
    feed_rate: Option<u32>,
}

#[derive(Args, Debug)]
struct TempArgs {
    #[arg(long)]
    id: i64,

    #[arg(long)]
    celsius: f64,
}

#[derive(Args, Debug)]
struct ExtrudeArgs {
    #[arg(long)]
    id: i64,

    #[arg(long)]
    mm: f64,

    #[arg(long)]
    feed_rate: Option<u32>,
}

#[derive(Serialize)]
struct CommandResponse {
    ok: bool,
    error: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli).await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let client = build_client(&cli)?;
    let base = cli.base_url.trim_end_matches('/');

    match cli.command {
        RootCommand::Printers(printers) => match printers.command {
            PrintersSubcommand::List => {
                let value = get_json(&client, &format!("{base}/api/printers")).await?;
                print_output(&value, cli.json)?;
            }
            PrintersSubcommand::Status(args) => {
                let value = get_json(&client, &format!("{base}/api/printers/{}/status", args.id)).await?;
                print_output(&value, cli.json)?;
            }
            PrintersSubcommand::Light(args) => {
                let on = match (args.on, args.off) {
                    (true, false) => true,
                    (false, true) => false,
                    _ => anyhow::bail!("specify exactly one of --on or --off"),
                };
                let payload = json!({ "type": "light", "on": on });
                let value = post_json(&client, &format!("{base}/api/printers/{}/command", args.id), &payload).await?;
                print_output(&value, cli.json)?;
            }
            PrintersSubcommand::Pause(args) => post_simple_command(&client, base, args.id, "pause", cli.json).await?,
            PrintersSubcommand::Resume(args) => post_simple_command(&client, base, args.id, "resume", cli.json).await?,
            PrintersSubcommand::Stop(args) => post_simple_command(&client, base, args.id, "stop", cli.json).await?,
            PrintersSubcommand::Home(args) => post_simple_command(&client, base, args.id, "home", cli.json).await?,
            PrintersSubcommand::Move(args) => {
                let payload = json!({
                    "type": "move",
                    "axis": format_axis(args.axis),
                    "distance": args.distance,
                    "feed_rate": args.feed_rate,
                });
                let value = post_json(&client, &format!("{base}/api/printers/{}/command", args.id), &payload).await?;
                print_output(&value, cli.json)?;
            }
            PrintersSubcommand::NozzleTemp(args) => {
                let payload = json!({ "type": "set_nozzle_temp", "target_c": args.celsius });
                let value = post_json(&client, &format!("{base}/api/printers/{}/command", args.id), &payload).await?;
                print_output(&value, cli.json)?;
            }
            PrintersSubcommand::BedTemp(args) => {
                let payload = json!({ "type": "set_bed_temp", "target_c": args.celsius });
                let value = post_json(&client, &format!("{base}/api/printers/{}/command", args.id), &payload).await?;
                print_output(&value, cli.json)?;
            }
            PrintersSubcommand::Extrude(args) => {
                let payload = json!({
                    "type": "extrude",
                    "amount_mm": args.mm,
                    "feed_rate": args.feed_rate,
                });
                let value = post_json(&client, &format!("{base}/api/printers/{}/command", args.id), &payload).await?;
                print_output(&value, cli.json)?;
            }
        },
    }

    Ok(())
}

fn build_client(cli: &Cli) -> anyhow::Result<Client> {
    let mut builder = Client::builder();
    if let Some(username) = &cli.username {
        let password = cli.password.clone().unwrap_or_default();
        let mut headers = reqwest::header::HeaderMap::new();
        let token = STANDARD.encode(format!("{username}:{password}"));
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Basic {token}"))?,
        );
        builder = builder.default_headers(headers);
    }
    Ok(builder.build()?)
}

async fn get_json(client: &Client, url: &str) -> anyhow::Result<Value> {
    let response = client.get(url).send().await?;
    parse_response(response).await
}

async fn post_json(client: &Client, url: &str, payload: &Value) -> anyhow::Result<Value> {
    let response = client.post(url).json(payload).send().await?;
    parse_response(response).await
}

async fn post_simple_command(client: &Client, base: &str, id: i64, kind: &str, json_output: bool) -> anyhow::Result<()> {
    let payload = json!({ "type": kind });
    let value = post_json(client, &format!("{base}/api/printers/{id}/command"), &payload).await?;
    print_output(&value, json_output)
}

async fn parse_response(response: reqwest::Response) -> anyhow::Result<Value> {
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        anyhow::bail!("request failed with status {}: {}", status, body);
    }

    if body.trim().is_empty() {
        return Ok(serde_json::to_value(CommandResponse { ok: true, error: None })?);
    }

    Ok(serde_json::from_str(&body).unwrap_or_else(|_| json!({ "raw": body })))
}

fn print_output(value: &Value, as_json: bool) -> anyhow::Result<()> {
    if as_json {
        println!("{}", serde_json::to_string(value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}

fn format_axis(axis: Axis) -> &'static str {
    match axis {
        Axis::X => "x",
        Axis::Y => "y",
        Axis::Z => "z",
    }
}
