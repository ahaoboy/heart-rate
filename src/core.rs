// main.rs
use btleplug::api::{Central, Manager as _, Peripheral as _};
use btleplug::platform::{Adapter, Manager};
use futures::stream::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use uuid::Uuid;

// Constants
const UUID_STR: &str = "00002a37-0000-1000-8000-00805f9b34fb";
const SUPPORT_DEVICES: &[&str] = &["Xiaomi Smart Band 9 082F"];

// Custom Error Type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Bluetooth adapters not found")]
    BluetoothAdaptersNotFound,
    #[error("Device not found")]
    DeviceNotFound,
    #[error("Heart rate characteristic not found")]
    CharacteristicNotFound,
    #[error("HeartRateMonitor not found")]
    HeartRateMonitorNotFound,
    #[error("Bluetooth error: {0}")]
    Bluetooth(#[from] btleplug::Error),
}

// Heart Rate Monitor Struct
pub struct HeartRateMonitor {
    adapter: Adapter,
    device_address: String,
}

impl HeartRateMonitor {
    pub async fn new(adapter: Adapter, device_address: String) -> Self {
        Self {
            adapter,
            device_address,
        }
    }

    pub async fn start_monitoring(&self) -> mpsc::Receiver<u8> {
        let (sender, receiver) = mpsc::channel(100);
        let adapter = self.adapter.clone();
        let device_address = self.device_address.clone();
        let adapter_arc = Arc::new(adapter);
        tokio::spawn({
            let adapter_arc = Arc::clone(&adapter_arc);
            async move {
                loop {
                    match Self::connect_and_monitor(&adapter_arc, &device_address, &sender).await {
                        Ok(_) => break,
                        Err(e) => {
                            eprintln!("Monitoring error: {e}");
                            time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        });

        receiver
    }

    async fn connect_and_monitor(
        adapter: &Adapter,
        device_address: &str,
        sender: &mpsc::Sender<u8>,
    ) -> Result<(), Error> {
        let heart_rate_uuid: Uuid = Uuid::parse_str(UUID_STR).expect("UUID_STR");
        adapter.start_scan(Default::default()).await?;
        time::sleep(Duration::from_secs(5)).await;

        let peripherals = adapter.peripherals().await?;
        let device = peripherals
            .into_iter()
            .find(|p| p.address().to_string() == device_address)
            .ok_or(Error::DeviceNotFound)?;

        device.connect().await?;
        device.discover_services().await?;

        let characteristics = device.characteristics();
        let hr_char = characteristics
            .into_iter()
            .find(|c| c.uuid == heart_rate_uuid)
            .ok_or(Error::CharacteristicNotFound)?;

        device.subscribe(&hr_char).await?;
        let mut notification_stream = device.notifications().await?;

        while let Some(data) = notification_stream.next().await {
            if data.uuid == heart_rate_uuid {
                let value = parse_heart_rate(&data.value);
                if sender.send(value).await.is_err() {
                    break; // Receiver closed
                }
            }
        }

        device.disconnect().await?;
        Ok(())
    }
}

fn parse_heart_rate(data: &[u8]) -> u8 {
    if data.len() >= 2 {
        if data[0] & 0x01 == 0 {
            data[1]
        } else {
            u16::from_le_bytes([data[1], data[2]]) as u8
        }
    } else {
        0
    }
}

async fn get_device_address(device_name: &str) -> Result<String, Error> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or(Error::BluetoothAdaptersNotFound)?;

    adapter.start_scan(Default::default()).await?;
    time::sleep(Duration::from_secs(5)).await;

    let peripherals = adapter.peripherals().await?;
    for peripheral in peripherals {
        let properties = peripheral.properties().await?;
        if let Some(props) = properties {
            if let Some(name) = props.local_name {
                if name == device_name {
                    let address = props.address.to_string();
                    adapter.stop_scan().await?;
                    return Ok(address);
                }
            }
        }
    }

    Err(Error::DeviceNotFound)
}

pub async fn detect_monitor() -> Result<HeartRateMonitor, Error> {
    for device_name in SUPPORT_DEVICES {
        match get_device_address(device_name).await {
            Ok(device_address) => {
                let manager = Manager::new().await?;
                let adapters = manager.adapters().await?;
                let adapter = adapters
                    .into_iter()
                    .next()
                    .ok_or(Error::BluetoothAdaptersNotFound)?;
                let monitor = HeartRateMonitor::new(adapter, device_address).await;
                return Ok(monitor);
            }
            Err(e) => eprintln!("Device not found: {device_name} - Error: {e}"),
        }
    }
    Err(Error::HeartRateMonitorNotFound)
}
