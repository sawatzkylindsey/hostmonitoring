#[tokio::main]
async fn main() {
    let parameters = hostmonitoring_agent::parameter::parse();
    hostmonitoring_agent::server::run_server(parameters).await
}
