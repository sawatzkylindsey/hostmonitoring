use hostmonitoring_agent;
use hyper::{body::Body, client::HttpConnector, Client, Request, StatusCode};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use tokio::task::JoinHandle;

const TEST_DATA: &str = "test-data";

// This test is expected to fail currently.
// TODO: Fix by implementing the server functionality.
#[tokio::test]
async fn inspect_file() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    let result = client.inspect("service.log").await.unwrap();

    // Verify
    assert_eq!(result, vec!["2 def".to_string(), "1 abc".to_string()]);
}

/// Starts the axum implementation of our host monitoring agent, rooted at `TEST_DATA`.
/// Returns a handle for the running agent, and a client that may be used to query the agent.
fn run_test_agent_instance() -> (JoinHandle<()>, AgentClient) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let server = tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .unwrap()
            .serve(
                hostmonitoring_agent::server::build_router(cwd.join(PathBuf::from(TEST_DATA)))
                    .into_make_service(),
            )
            .await
            .unwrap();
    });

    (server, AgentClient::new(address))
}

struct AgentClient {
    client: Client<HttpConnector, Body>,
    address: SocketAddr,
}

impl AgentClient {
    fn new(address: SocketAddr) -> Self {
        let client: Client<HttpConnector, Body> = Client::builder().build_http();
        Self { client, address }
    }

    async fn inspect(&self, path: impl Into<String>) -> Result<Vec<String>, StatusCode> {
        let request = Request::builder()
            .uri(format!(
                "http://{address}/inspect/{path}",
                address = self.address,
                path = path.into(),
            ))
            .body(Body::empty())
            .expect("must be a valid hyper::Request");
        match self.client.request(request).await {
            Ok(response) => {
                if response.status().is_success() {
                    let logs: Vec<String> = serde_json::from_slice(
                        &hyper::body::to_bytes(response.into_body())
                            .await
                            .expect("response body must convert to bytes"),
                    )
                    .expect("json must deserialize into Vec<String>");
                    Ok(logs)
                } else {
                    Err(response.status())
                }
            }
            Err(e) => panic!(
                "error connecting to {address}: {e}",
                address = self.address,
                e = e
            ),
        }
    }
}
