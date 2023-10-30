
use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(name="roboscapesim-testclient", version="0.1.0", about="Test client for RoboScape Online")]
struct Args {
    num_clients: usize,

    scenario: Option<String>,

    #[arg(short = 'r', long)]
    roboscape_online_server: Option<String>,

    #[arg(short = 'n', long)]
    netsblox_services_server: Option<String>,

    #[arg(short = 'l', long)]
    netsblox_cloud_server: Option<String>,
}

fn main() {
    let mut args = Args::parse();

    if args.roboscape_online_server.is_none() {
        args.roboscape_online_server = Some("http://localhost:5001".to_owned());
    }

    if args.netsblox_services_server.is_none() {
        args.netsblox_services_server = Some("http://localhost:8080".to_owned());
    }

    if args.netsblox_cloud_server.is_none() {
        args.netsblox_cloud_server = Some("http://localhost:7777".to_owned());
    }

    if args.scenario.is_none() {
        args.scenario = Some("Default".to_owned());
    }

    println!("{:?}", args);
}
