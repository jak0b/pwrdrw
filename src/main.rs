// Based on original code by Luyer: https://apple.stackexchange.com/a/283403
// Modified by M-1.

use std::{
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Mutex,
    time::Duration,
};

use clap::{Args as ClapArgs, Parser, Subcommand};
use power_reading::SmcConnection;
use tokio::time::sleep;

mod metrics;
mod power_reading;
mod server;

#[cfg(not(target_os = "macos"))]
compile_error!("pwrdrw can only be built for macOS");

#[derive(Debug, Parser)]
#[command(about = "Read power draw from the macOS AppleSMC service")]
struct Args {
    /// Continuously print readings
    #[arg(short = 'l', long = "loop")]
    looping: bool,

    /// Clear the terminal before each reading
    #[arg(short = 'c', long = "clear")]
    clear_screen: bool,

    /// Delay between readings in milliseconds
    #[arg(short = 'r', long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(1..=2147483647))]
    interval: u64,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve Prometheus metrics over HTTP
    Metrics(MetricsArgs),
}

#[derive(Debug, ClapArgs)]
#[command(disable_help_flag = true)]
struct MetricsArgs {
    /// Print help
    #[arg(short = 'H', long = "help", action = clap::ArgAction::Help)]
    help: Option<bool>,

    /// Port to serve metrics on
    #[arg(short = 'p', long, default_value_t = 9100, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,

    /// Delay between AppleSMC readings in milliseconds
    #[arg(short = 'l', long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(1..=2147483647))]
    poll_interval: u64,

    /// Comma-separated IP addresses to serve metrics on
    #[arg(short = 'h', long, value_delimiter = ',')]
    hosts: Option<Vec<IpAddr>>,
}

async fn print_readings(smc: &Mutex<SmcConnection>) -> Result<(), Box<dyn Error>> {
    let readings = smc
        .lock()
        .map_err(|_| io::Error::other("AppleSMC lock was poisoned"))?
        .read_power()?;
    println!(
        "Power Source: {}",
        if readings.on_ac_power {
            "AC Power"
        } else {
            "Battery"
        }
    );
    println!(
        "DC In: {:.6}V {:.6}A {:.6}W",
        readings.voltage,
        readings.current,
        readings.voltage * readings.current
    );
    println!("System Total: {:.6}W", readings.system_total);
    match readings.battery_percentage {
        Some(percentage) => println!("Battery Charge: {percentage:.1}%"),
        None => println!("Battery Charge: unavailable"),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let smc = SmcConnection::open()?;

    if let Some(Command::Metrics(metrics_args)) = args.command {
        let hosts = metrics_args.hosts.unwrap_or_else(|| {
            vec![
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            ]
        });
        return server::serve(metrics_args.port, metrics_args.poll_interval, hosts, smc).await;
    }

    let smc = Mutex::new(smc);
    loop {
        if args.looping && args.clear_screen {
            print!("\x1b[H\x1b[2J");
        }
        print_readings(&smc).await?;
        if !args.looping {
            return Ok(());
        }
        sleep(Duration::from_millis(args.interval)).await;
    }
}
