use futures::future::{join_all, JoinAll};
use hostmonitoring_agent;
use hyper::{body::Body, client::HttpConnector, Client, Request, StatusCode};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;

const TEST_DATA: &str = "test-data";

#[tokio::test]
async fn inspect_file() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    let result = client.inspect("service.log").await.unwrap();

    // Verify
    assert_eq!(
        result,
        vec!["".to_string(), "2 def".to_string(), "1 abc".to_string()]
    );
}

#[tokio::test]
async fn inspect_not_found() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    let result = client.inspect("not-found.log").await.unwrap_err();

    // Verify
    assert_eq!(result.expect_server(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn inspect_relative_path() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    // This is a valid path, but out of the intendend root log directory.
    let result = client.inspect("../Cargo.toml").await.unwrap_err();

    // Verify
    assert_eq!(result.expect_server(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn inspect_non_utf8() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    // File generated with:
    //  echo -ne "\x0a\xed\x9f\xbf" > test-data/non-utf8.log
    let result = client.inspect("non-utf8.log").await.unwrap();

    // Verify
    assert_eq!(result, vec!["\u{d7ff}".to_string()]);
}

#[tokio::test]
async fn inspect_invalid_utf8() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    // File generated with:
    //  echo -ne "\x0a\x80" > test-data/invalid-utf8.log
    let result = client.inspect("invalid-utf8.log").await.unwrap_err();

    // Verify
    // With the streaming change we're relying on a 3rd party crate (a package) to produce a stream of json.
    // Unfortunately, this crate doesn't give us the ability to instrument what to do when encountering an error.
    // I left this as the server cutting the stream, which is what we see here.
    assert!(result.expect_runtime().contains("unexpected EOF"));
}

#[tokio::test]
async fn inspect_long() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    // File generated in build.rs.
    let result = client.inspect("long.log").await.unwrap();

    // Verify
    assert_eq!(
        result,
        (0..100_000)
            .rev()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn inspect_wide() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    // File generated in build.rs.
    let result = client.inspect("wide.log").await.unwrap();

    // Verify
    assert_eq!(result, vec!["a".repeat(100_000)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn inspect_concurrent_requests() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    let results: Vec<Result<Vec<String>, TestClientError>> = join_all((0..100).map(|_| async {
        let client_clone = client.clone();
        tokio::spawn(async move {
            // File generated in build.rs.
            client_clone.inspect("long.log").await
        })
        .await
        .unwrap()
    }))
    .await;

    // Verify
    let expected = (0..100_000)
        .rev()
        .map(|i| i.to_string())
        .collect::<Vec<String>>();
    for result in results {
        match result {
            Ok(lines) => {
                assert_eq!(lines, expected);
            }
            Err(error) => {
                match error.expect_server() {
                    StatusCode::SERVICE_UNAVAILABLE => {
                        // The server/OS can only take so many open files at once - this one is OK.
                    }
                    _ => panic!("{error:?}"),
                }
            }
        }
    }
}

#[tokio::test]
#[ignore] // Takes about 1.5 minutes & less than 8 MB on the hostmonitoring-agent on my computer.
async fn inspect_large() {
    // Setup
    let (_server, client) = run_test_agent_instance();

    // Execute
    // File generated in build.rs.
    let result = client.inspect("large.log").await.unwrap();

    // Verify
    assert_eq!(result.len(), 100_000);
}

/// Starts the axum implementation of our host monitoring agent, rooted at `TEST_DATA`.
/// Returns a handle for the running agent, and a client that may be used to query the agent.
fn run_test_agent_instance() -> (JoinAll<JoinHandle<()>>, AgentClient) {
    let (sender, mut receiver) = unbounded_channel::<JoinHandle<()>>();
    let driver = tokio::spawn(async move {
        loop {
            if let Some(log_file_read_task) = receiver.recv().await {
                log_file_read_task
                    .await
                    .expect("task must complete successfully");
            }
        }
    });

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let server = tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .unwrap()
            .serve(
                hostmonitoring_agent::server::build_router(
                    sender,
                    cwd.join(PathBuf::from(TEST_DATA)),
                )
                .into_make_service(),
            )
            .await
            .unwrap();
    });

    let threads = join_all(vec![server, driver]);
    (threads, AgentClient::new(address))
}

#[derive(Clone)]
struct AgentClient {
    client: Client<HttpConnector, Body>,
    address: SocketAddr,
}

impl AgentClient {
    fn new(address: SocketAddr) -> Self {
        let client: Client<HttpConnector, Body> = Client::builder().build_http();
        Self { client, address }
    }

    async fn inspect(&self, path: impl Into<String>) -> Result<Vec<String>, TestClientError> {
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
                    hyper::body::to_bytes(response.into_body())
                        .await
                        .map(|bytes| {
                            serde_json::from_slice(&bytes)
                                .expect("json must deserialize into Vec<String>")
                        })
                        .map_err(|error| TestClientError::runtime_error(format!("{error}")))
                } else {
                    Err(TestClientError::server_error(response.status()))
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

#[derive(Debug, PartialEq, Eq)]
struct TestClientError {
    status_code: Option<StatusCode>,
    message: Option<String>,
}

impl TestClientError {
    fn server_error(status_code: StatusCode) -> Self {
        Self {
            status_code: Some(status_code),
            message: None,
        }
    }

    fn runtime_error(message: String) -> Self {
        Self {
            status_code: None,
            message: Some(message),
        }
    }

    fn expect_server(&self) -> StatusCode {
        self.status_code.unwrap()
    }

    fn expect_runtime(&self) -> String {
        self.message.clone().unwrap()
    }
}
