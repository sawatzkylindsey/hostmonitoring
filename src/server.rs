use axum::body::Body;
use axum::http::Request;
use axum::{http::StatusCode, Json, Router};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::parameter::Parameters;

/// Run the agent service using the specified parameters.
/// This starts an HTTP server and runs indefinitely (must use CTRL+C to exit).
pub async fn run_server(parameters: Parameters) {
    let router = Router::new().nest("/inspect", Router::new().fallback(inspect_log));

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), parameters.port);
    axum::Server::bind(&address)
        .serve(router.into_make_service())
        .await
        .unwrap();
}

/// Inspect the `/var/log` file specified by the request path.
///
/// If the request path specifies a valid file within `/var/log`, then its contents are returned as
/// an `application/json` list of strings in reverse chronological order.
/// Otherwise, an HTTP error is returned.
pub async fn inspect_log(request: Request<Body>) -> Result<Json<Vec<String>>, StatusCode> {
    let filepath = request.uri().path().trim_start_matches('/');

    // TODO: Implement the actual log inspection functionality.
    println!("reading: {filepath}");
    Ok(Json(vec!["pretend1".to_string(), "pretend2".to_string()]))
}
