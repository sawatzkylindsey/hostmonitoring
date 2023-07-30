#[tokio::main]
async fn main() {
    let parameters = hostmonitoring_agent::parameter::parse();

    // Prevent from shooting ourselves in the foot by specifying a non-absolute log root.
    // If we didn't do this, then requests would fail (but it would look like a client-side error).
    if !parameters.log_root.is_absolute() {
        panic!(
            "Invalid log_root: {}",
            parameters.log_root.to_string_lossy()
        );
    }

    hostmonitoring_agent::server::run_server(parameters).await
}
