use crate::parameter::Parameters;
use crate::read::reverse::{reverse_read_runner, ChannelReceiverStream};
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::response::IntoResponse;
use axum::{http::StatusCode, Router};
use axum_streams::StreamBodyAs;
use futures::future::join_all;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::sync::mpsc::{channel, unbounded_channel, UnboundedSender};
use tokio::task::JoinHandle;

/// The maximum number of log lines that can go onto a file reader task.
/// This is a per-request limit.
const LOG_CHANNEL_SIZE: usize = 1_000;

/// Run the agent service using the specified parameters.
/// This starts an HTTP server and runs indefinitely (must use CTRL+C to exit).
pub async fn run_server(parameters: Parameters) {
    let Parameters { port, log_root } = parameters;

    // Setup a background task which will drive the log file reads asynchronously.
    // We're going to make a channel with a sender + receiver.
    // The /inspect handler is going to build file read tasks and put them onto this channel.
    // Here, we build a driver that takes the receiver end of that channel and runs those file read tasks to completion.
    // We use an unbounded channel to allow unlimited simultaneous requests (there may be various practical resource limitations, but
    // in terms of running the background tasks we don't enforce a limitation).
    // This is something that could be tweaked in the future (ex: maybe we reject the request if too many come in).
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

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
    // Setup the axum server, but don't run it yet.
    let server = tokio::spawn(async move {
        axum::Server::bind(&address)
            .serve(build_router(sender, log_root).into_make_service())
            .await
            .unwrap();
    });

    // Finally, our program needs to join both our axum server and the background driver.
    join_all(vec![server, driver]).await;
}

/// Container struct to pass state into the /inspect handler.
#[derive(Clone)]
pub(crate) struct HandlerState {
    task_sender: Arc<UnboundedSender<JoinHandle<()>>>,
    log_root: PathBuf,
}

/// Build the host monitoring agent router.
/// This is provided for integration testing.
pub fn build_router(task_sender: UnboundedSender<JoinHandle<()>>, log_root: PathBuf) -> Router {
    Router::new().nest(
        "/inspect",
        Router::new()
            .fallback(inspect_log)
            .with_state(HandlerState {
                task_sender: Arc::new(task_sender),
                log_root,
            }),
    )
}

/// Inspect the file rooted at `log_root` and specified by the request path.
///
/// If the request path specifies a valid file within `log_root`, then its contents are returned as
/// an `application/json` list of strings in reverse chronological order.
/// Otherwise, an HTTP error is returned.
pub(crate) async fn inspect_log(
    State(state): State<HandlerState>,
    request: Request<Body>,
) -> impl IntoResponse {
    let filepath = request.uri().path().trim_start_matches('/');
    let filepath = PathBuf::try_from(filepath).unwrap();
    let absolute_filepath = state.log_root.join(filepath);
    println!("reading: {}", absolute_filepath.to_string_lossy());

    match absolute_filepath.canonicalize() {
        Ok(filepath_canonical) => {
            // Make sure the specified path doesn't walk "up" the directory tree.
            // Axum already seems to strip any relative paths, but this protects in case that ever goes wrong.
            if absolute_filepath == filepath_canonical {
                match File::open(filepath_canonical).await {
                    Ok(file) => {
                        // Setup to allow for the log lines to be streamed asynchronously.
                        // We're going to make a channel with a sender + receiver.
                        // In the background, we'll process the log file putting items on the channel when they become available.
                        // Then this method will return a stream that reads off the channel and serializes into a json array of strings.
                        let (sender, receiver) = channel(LOG_CHANNEL_SIZE);
                        let task = tokio::spawn(reverse_read_runner(sender, Box::new(file)));
                        // Send the background task to our top level driver.
                        state
                            .task_sender
                            .send(task)
                            .expect("driver channel must still be open");
                        let receiver_stream = ChannelReceiverStream::new(receiver);
                        StreamBodyAs::json_array(receiver_stream).into_response()
                    }
                    Err(error) => {
                        if let Some(os_error) = error.raw_os_error() {
                            if os_error == 24 && error.to_string().contains("Too many open files") {
                                return StatusCode::SERVICE_UNAVAILABLE.into_response();
                            }
                        }

                        match error.kind() {
                            ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
                            // Let's map anything un-accounted for as an internal error.
                            // We may refine this over time as cases are discovered.
                            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                        }
                    }
                }
            } else {
                StatusCode::BAD_REQUEST.into_response()
            }
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
