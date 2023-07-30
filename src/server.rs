use crate::parameter::Parameters;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::{http::StatusCode, Json, Router};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use tokio::fs::File;

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
    let filepath = PathBuf::try_from(filepath).unwrap();
    let absolute_filepath = log_root.join(filepath);
    println!("reading: {}", absolute_filepath.to_string_lossy());

    match absolute_filepath.canonicalize() {
        Ok(filepath_canonical) => {
            // Make sure the specified path doesn't walk "up" the directory tree.
            // Axum already seems to strip any relative paths, but this protects in case that ever goes wrong.
            if absolute_filepath == filepath_canonical {
                match File::open(filepath_canonical).await {
                    Ok(mut _file) => {
                        Ok(Json(vec!["pretend1".to_string(), "pretend2".to_string()]))
                        // TODO: Implement FileLike for tokio::fs::File.
                        //Ok(Json(reverse_read(&mut file).await))
                    }
                    Err(_) => Err(StatusCode::NOT_FOUND),
                }
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}
