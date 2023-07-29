mod parameter;

#[tokio::main]
async fn main() {
    let parameters = parameter::parse();
    println!("Running with {parameters:?}");
}
