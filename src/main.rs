#![deny(unsafe_code)]

mod canonicalise;
mod dispatch;
mod err;
mod models;
mod program_args;
mod request;
mod response;
mod server;

fn main() -> err::Result {
    let args = program_args::parse();
    if args.print_version {
        let version = env!("CARGO_PKG_VERSION");
        println!("Incognita Socket server version {version}");
        std::process::exit(0);
    }
    
    let server = server::Server::new(args.max_connections);
    async_std::task::block_on(dispatch::start_server(server, "0.0.0.0", args.port))
}
