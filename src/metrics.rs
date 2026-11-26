use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use metrics::{describe_gauge, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tokio::time::MissedTickBehavior;

use crate::power_reading::{Readings, SmcConnection};

#[derive(Clone)]
pub struct Metrics {
    readings_available: Arc<AtomicBool>,
    prometheus: PrometheusHandle,
}

impl Metrics {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let prometheus = PrometheusBuilder::new().install_recorder()?;
        describe_metrics();

        Ok(Self {
            readings_available: Arc::new(AtomicBool::new(false)),
            prometheus,
        })
    }

    pub fn render(&self) -> Option<String> {
        self.readings_available
            .load(Ordering::Relaxed)
            .then(|| self.prometheus.render())
    }

    fn record(&self, readings: Readings) {
        gauge!("macos_power_input_voltage_volts").set(f64::from(readings.voltage));
        gauge!("macos_power_input_current_amperes").set(f64::from(readings.current));
        gauge!("macos_power_input_watts")
            .set(f64::from(readings.voltage) * f64::from(readings.current));
        gauge!("macos_system_power_watts").set(f64::from(readings.system_total));
        gauge!("macos_power_source_connected").set(if readings.on_ac_power { 1.0 } else { 0.0 });
        if let Some(percentage) = readings.battery_percentage {
            gauge!("macos_battery_charge_percentage").set(f64::from(percentage));
        }
        self.readings_available.store(true, Ordering::Relaxed);
    }

    fn mark_unavailable(&self) {
        self.readings_available.store(false, Ordering::Relaxed);
    }
}

pub fn start_polling(smc: SmcConnection, poll_interval: Duration, metrics: Metrics) {
    let smc = Arc::new(Mutex::new(smc));

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            let smc = Arc::clone(&smc);
            let result = tokio::task::spawn_blocking(move || {
                let smc = smc
                    .lock()
                    .map_err(|_| std::io::Error::other("AppleSMC lock was poisoned"))?;
                smc.read_power()
            })
            .await;

            match result {
                Ok(Ok(readings)) => metrics.record(readings),
                Ok(Err(error)) => {
                    eprintln!("Unable to read AppleSMC: {error}");
                    metrics.mark_unavailable();
                }
                Err(error) => {
                    eprintln!("AppleSMC polling task failed: {error}");
                    metrics.mark_unavailable();
                }
            }
        }
    });
}

fn describe_metrics() {
    describe_gauge!(
        "macos_power_input_voltage_volts",
        "DC input voltage in volts."
    );
    describe_gauge!(
        "macos_power_input_current_amperes",
        "DC input current in amperes."
    );
    describe_gauge!("macos_power_input_watts", "DC input power in watts.");
    describe_gauge!("macos_system_power_watts", "Total system power in watts.");
    describe_gauge!(
        "macos_power_source_connected",
        "Whether an external power source is connected (1) or the Mac is using battery power (0)."
    );
    describe_gauge!(
        "macos_battery_charge_percentage",
        "Battery charge as a percentage of full charge capacity."
    );
}
