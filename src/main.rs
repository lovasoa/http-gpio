use std::sync::Arc;
use serde::Serialize;
use warp::{Filter, Reply};
use warp::http::StatusCode;
use warp::reply::{json, with_status};

use application_state::{AppResult, GpioPath, State};

mod application_state;

#[tokio::main]
async fn main() {
    let shared_pins_state = Arc::new(State::new());
    let with_pins_state = warp::any().map(move || shared_pins_state.clone());

    let gpio_hello = warp::path!("gpio")
        .map(|| "This is the GPIO API");

    let gpio_modify = warp::post()
        .and(warp::path!("gpio" / String / u32))
        .map(GpioPath::new)
        .and(with_pins_state)
        .and(warp::body::content_length_limit(10))
        .and(warp::body::json())
        .map(|gpio_path, state: Arc<State>, body| state.write(gpio_path, body))
        .map(create_http_response);

    let routes = gpio_hello.or(gpio_modify);

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}

fn create_http_response<O: Serialize>(r: AppResult<O>) -> impl warp::Reply {
    let status = if r.is_err() { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::OK };
    let body: Box<dyn Reply> = r
        .map(|o| Box::new(json(&o)) as Box<dyn Reply>)
        .unwrap_or_else(|err| Box::new(err.to_string()));
    with_status(body, status)
}