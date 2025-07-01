use heart_rate::create_monitor;

#[tokio::main]
async fn main() {
    let Ok(monitor) = create_monitor().await else {
        return;
    };
    let mut receiver = monitor.start_monitoring().await;
    while let Some(hr) = receiver.recv().await {
        println!("{hr}");
    }
}
