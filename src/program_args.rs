use arg::Args;

#[derive(Args)]
///incognita-socket-server
///Runs a server for the Incognita Socket protocol.
pub(crate) struct ProgramArgs {
    #[arg(long = "version")]
    ///Print version number and then exit
    pub(crate) print_version: bool,
    
    #[arg(short, long, default_value = "31337")]
    ///Listen on this port
    pub(crate) port: u16,
    
    #[arg(long = "max-connections", default_value = "256")]
    pub(crate) max_connections: usize,
}

pub(crate) fn parse() -> ProgramArgs {
    arg::parse_args()
}
