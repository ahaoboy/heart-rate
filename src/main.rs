use heart_rate::detect_monitor;

#[tokio::main]
async fn main() {
    let Ok(monitor) = detect_monitor().await else {
        return;
    };
    let mut receiver = monitor.start_monitoring().await;
    while let Some(hr) = receiver.recv().await {
        println!("{hr}");
    }
}
