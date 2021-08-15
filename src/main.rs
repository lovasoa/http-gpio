use std::sync::Arc;

use log::error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use structopt::StructOpt;
use warp::{Filter, Rejection};
use warp::http::StatusCode;
use warp::hyper::body::Bytes;
use warp::reply::{json, with_status};

use application_state::{AppResult, GpioPath, State};
use application_state::{list_chips, list_pins, single_pin_description};
use command_line_arguments::CommandLineArguments;

mod application_state;
mod command_line_arguments;

#[tokio::main]
async fn main() {
    let opts = CommandLineArguments::from_args();
    env_logger::Builder::from_env(env_logger::Env::default()
        .filter_or("LOG", opts.log)
    ).init();

    let cors = warp::cors()
        .allow_origins(opts.allow_origin.iter().map(String::as_str))
        .allow_methods(["GET", "POST"])
        .build();

    let shared_pins_state = Arc::new(State::new());
    let routes =
        gpio_list()
            .or(gpio_pin_list())
            .or(gpio_pin_description())
            .or(gpio_get(shared_pins_state.clone()))
            .or(gpio_post(shared_pins_state.clone()))
            .or(gpio_blink(shared_pins_state.clone()))
            .with(warp::log("http-gpio"))
            .with(cors);

    warp::serve(routes).run(opts.bind).await;
}

type StateRef = Arc<State>;

fn gpio_list() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("gpio")
        .map(list_chips)
        .map(create_http_response)
}

fn gpio_pin_list() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("gpio" / String)
        .map(list_pins)
        .map(create_http_response)
}

fn gpio_pin_description() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("gpio" / String / u32)
        .map(GpioPath::new)
        .map(single_pin_description)
        .map(create_http_response)
}

fn gpio_child_path(
    shared_pins_state: StateRef,
    child: &'static str,
) -> impl Filter<Extract=(GpioPath, StateRef), Error=warp::Rejection> + Clone {
    warp::path("gpio")
        .and(warp::path::param::<String>())
        .and(warp::path::param::<u32>())
        .and(warp::path::path(child))
        .and(warp::path::end())
        .map(GpioPath::new)
        .and(warp::any().map(move || shared_pins_state.clone()))
}

fn gpio_post(
    state: StateRef,
) -> impl Filter<Extract=impl warp::Reply, Error=warp::Rejection> + Clone {
    gpio_child_path(state, "value")
        .and(warp::post())
        .and(any_json())
        .map(|gpio_path, state: Arc<State>, body| state.write(gpio_path, body))
        .map(create_http_response)
}

fn gpio_blink(
    state: StateRef,
) -> impl Filter<Extract=impl warp::Reply, Error=warp::Rejection> + Clone {
    gpio_child_path(state, "blink")
        .and(warp::post())
        .and(warp::body::content_length_limit(4096))
        .and(any_json())
        .map(|gpio_path, state: Arc<State>, body| state.write_schedule(gpio_path, body))
        .map(create_http_response)
}

fn gpio_get(
    state: StateRef,
) -> impl Filter<Extract=impl warp::Reply, Error=warp::Rejection> + Clone {
    gpio_child_path(state, "value")
        .and(warp::get())
        .map(|gpio_path, state: Arc<State>| state.read(gpio_path))
        .map(create_http_response)
}

fn create_http_response<O: Serialize>(r: AppResult<O>) -> Box<dyn warp::Reply> {
    match r {
        Ok(value) => Box::new(json(&value)),
        Err(e) => {
            error!("{}", e);
            Box::new(with_status(
                e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub fn any_json<T: DeserializeOwned + Send>() -> impl Filter<Extract=(T, ), Error=Rejection> + Copy {
    warp::filters::body::bytes()
        .and_then(|buf: Bytes| async move {
            serde_json::from_slice(&buf).map_err(|err| {
                error!("request json body error: {}", err);
                warp::reject::reject()
            })
        })
}