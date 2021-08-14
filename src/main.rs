use std::net::SocketAddr;
use std::sync::Arc;

use serde::Serialize;
use warp::{Filter, Reply};
use warp::http::StatusCode;
use warp::reply::{json, with_status};

use application_state::{AppResult, GpioPath, State};

mod application_state;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(
    env_logger::Env::default().filter_or("LOG", "INFO")
    ).init();

    let shared_pins_state = Arc::new(State::new());
    let routes =
        gpio_get(shared_pins_state.clone())
            .or(gpio_post(shared_pins_state.clone()))
            .with(warp::log("http-gpio"));

    let addr: SocketAddr = ([127, 0, 0, 1], 3030).into();
    warp::serve(routes)
        .run(addr)
        .await;
}

type StateRef = Arc<State>;

fn gpio_path(shared_pins_state: StateRef) -> impl Filter<Extract=(GpioPath, StateRef,), Error=warp::Rejection> + Clone {
    warp::path!("gpio" / String / u32)
        .map(GpioPath::new)
        .and(warp::any().map(move || shared_pins_state.clone()))
}


fn gpio_post(state: StateRef) -> impl Filter<Extract=impl warp::Reply, Error=warp::Rejection> + Clone {
    gpio_path(state)
        .and(warp::post())
        .and(warp::body::content_length_limit(10))
        .and(warp::body::json())
        .map(|gpio_path, state: Arc<State>, body| state.write(gpio_path, body))
        .map(create_http_response)
}

fn gpio_get(state: StateRef) -> impl Filter<Extract=impl warp::Reply, Error=warp::Rejection> + Clone {
    gpio_path(state)
        .and(warp::get())
        .map(|gpio_path, state: Arc<State>| state.read(gpio_path))
        .map(create_http_response)
}

fn create_http_response<O: Serialize>(r: AppResult<O>) -> impl warp::Reply {
    let status = if r.is_err() { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::OK };
    let body: Box<dyn Reply> = r
        .map(|o| Box::new(json(&o)) as Box<dyn Reply>)
        .unwrap_or_else(|err| Box::new(err.to_string()));
    with_status(body, status)
}