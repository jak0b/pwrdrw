use std::{error::Error, io, sync::Mutex, time::Duration};

use clap::Parser;
use power_reading::SmcConnection;
use tokio::time::sleep;

mod metrics;
mod power_reading;

#[cfg(not(target_os = "macos"))]
compile_error!("power-usage-macos can only be built for macOS");

#[derive(Debug, Parser)]
#[command(about = "Read power usage from the macOS AppleSMC service")]
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

    /// Serve Prometheus metrics on this port
    #[arg(
        short = 'p',
        long,
        value_parser = clap::value_parser!(u16).range(1..),
        conflicts_with_all = ["looping", "clear_screen", "interval"]
    )]
    port: Option<u16>,
}

async fn print_readings(smc: &Mutex<SmcConnection>) -> Result<(), Box<dyn Error>> {
    let readings = smc
        .lock()
        .map_err(|_| io::Error::other("AppleSMC lock was poisoned"))?
        .read_power()?;
    println!(
        "DC In: {:.6}V {:.6}A {:.6}W",
        readings.voltage,
        readings.current,
        readings.voltage * readings.current
    );
    println!("System Total: {:.6}W", readings.system_total);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let smc = SmcConnection::open()?;

    if let Some(port) = args.port {
        return metrics::serve(port, smc).await;
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
