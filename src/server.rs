use crate::parameter::Parameters;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::{http::StatusCode, Json, Router};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Run the agent service using the specified parameters.
/// This starts an HTTP server and runs indefinitely (must use CTRL+C to exit).
pub async fn run_server(parameters: Parameters) {
    let Parameters { port, log_root } = parameters;
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
    axum::Server::bind(&address)
        .serve(build_router(log_root).into_make_service())
        .await
        .unwrap();
}

/// Build the host monitoring agent router.
/// This is provided for integration testing.
pub fn build_router(log_root: PathBuf) -> Router {
    Router::new().nest(
        "/inspect",
        Router::new().fallback(inspect_log).with_state(log_root),
    )
}

/// Inspect the file rooted at `log_root` and specified by the request path.
///
/// If the request path specifies a valid file within `log_root`, then its contents are returned as
/// an `application/json` list of strings in reverse chronological order.
/// Otherwise, an HTTP error is returned.
pub async fn inspect_log(
    State(log_root): State<PathBuf>,
    request: Request<Body>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let filepath = request.uri().path().trim_start_matches('/');

    // TODO: Implement the actual log inspection functionality.

    // Prints output like:
    //   reading: /var/log/system.log
    println!(
        "reading: {log_root}/{filepath}",
        log_root = log_root.to_string_lossy()
    );
    Ok(Json(vec!["pretend1".to_string(), "pretend2".to_string()]))
}
