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

use crate::power_reading::{PowerReader, Readings};

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
        #[cfg(target_os = "macos")]
        let names = (
            "macos_power_input_voltage_volts",
            "macos_power_input_current_amperes",
            "macos_power_input_watts",
            "macos_system_power_watts",
            "macos_power_source_connected",
            "macos_battery_charge_percentage",
        );
        #[cfg(target_os = "linux")]
        let names = (
            "linux_power_input_voltage_volts",
            "linux_power_input_current_amperes",
            "linux_power_input_watts",
            "linux_system_power_watts",
            "linux_power_source_connected",
            "linux_battery_charge_percentage",
        );

        gauge!(names.0).set(f64::from(readings.voltage));
        gauge!(names.1).set(f64::from(readings.current));
        gauge!(names.2).set(f64::from(readings.voltage) * f64::from(readings.current));
        gauge!(names.3).set(f64::from(readings.system_total));
        gauge!(names.4).set(if readings.on_ac_power { 1.0 } else { 0.0 });
        if let Some(percentage) = readings.battery_percentage {
            gauge!(names.5).set(f64::from(percentage));
        }
        self.readings_available.store(true, Ordering::Relaxed);
    }

    fn mark_unavailable(&self) {
        self.readings_available.store(false, Ordering::Relaxed);
    }
}

pub fn start_polling(smc: PowerReader, poll_interval: Duration, metrics: Metrics) {
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
                    .map_err(|_| std::io::Error::other("Power sensor lock was poisoned"))?;
                smc.read_power()
            })
            .await;

            match result {
                Ok(Ok(readings)) => metrics.record(readings),
                Ok(Err(error)) => {
                    eprintln!("Unable to read power sensors: {error}");
                    metrics.mark_unavailable();
                }
                Err(error) => {
                    eprintln!("Power sensor polling task failed: {error}");
                    metrics.mark_unavailable();
                }
            }
        }
    });
}

fn describe_metrics() {
    #[cfg(target_os = "macos")]
    let names = (
        "macos_power_input_voltage_volts",
        "macos_power_input_current_amperes",
        "macos_power_input_watts",
        "macos_system_power_watts",
        "macos_power_source_connected",
        "macos_battery_charge_percentage",
    );
    #[cfg(target_os = "linux")]
    let names = (
        "linux_power_input_voltage_volts",
        "linux_power_input_current_amperes",
        "linux_power_input_watts",
        "linux_system_power_watts",
        "linux_power_source_connected",
        "linux_battery_charge_percentage",
    );

    describe_gauge!(names.0, "DC input voltage in volts.");
    describe_gauge!(names.1, "DC input current in amperes.");
    describe_gauge!(names.2, "DC input power in watts.");
    describe_gauge!(names.3, "Total system power in watts.");
    describe_gauge!(
        names.4,
        "Whether an external power source is connected (1) or the system is using battery power (0)."
    );
    describe_gauge!(
        names.5,
        "Battery charge as a percentage of full charge capacity."
    );
}
