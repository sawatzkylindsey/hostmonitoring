mod parameter;
mod server;

#[tokio::main]
async fn main() {
    let parameters = parameter::parse();
    server::run_server(parameters).await
}
