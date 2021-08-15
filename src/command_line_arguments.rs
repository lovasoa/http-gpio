use structopt::StructOpt;
use std::net::SocketAddr;

/// A program which launches a web server to control a machine's GPIO pins over HTTP
#[derive(StructOpt, Debug)]
#[structopt(name = "http_gpio")]
pub struct CommandLineArguments {
    /// How verbose the logs should be.
    /// Set it to "debug" to make the program log everything it is doing.
    /// Set it to "info,http_gpio=debug" to get detailed information only about http_gpio itself,
    /// and not external libraries.
    #[structopt(short, long, default_value = "info")]
    pub log: String,

    /// The network interface and port to expose the HTTP server on
    #[structopt(short, long, default_value = "0.0.0.0:3030")]
    pub bind: SocketAddr,

    /// Which websites to give access to the exposed server.
    /// By default, a web page loaded in your browser can only make requests to its own origin.
    /// By using this argument, you allow web pages loaded in browsers which have access to this server
    /// to interact with it.
    /// Set this to "https://example.com" and you will be able to call this server from javascript
    /// code on any page of example.com.
    #[structopt(short, long)]
    pub allow_origin: Vec<String>,
}