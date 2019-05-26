use snafu::{Snafu};


#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum Error {
    #[snafu(display("IO Error: {}", source))]
    IO { source: std::io::Error },

    #[snafu(display("Serde Serialize Error: {}", source))]
    SerdeSerializationError { source: serde_json::error::Error },

    /*#[snafu(display("From String Error: {}", source))]
    SerdeSerializationError { source: std::str::From },
*/
    #[snafu(display("Not enough input provided"))]
    UnsufficientInput {  },

    #[snafu(display("Clock could not be found"))]
    ClockNotFound {  },

    #[snafu(display("Clock is out of index"))]
    ClockOutOfIndex {  },

    #[snafu(display("Parsing Error: {}", source))]
    ChronoParseError { source: chrono::format::ParseError },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;



#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum CliError {
    #[snafu(display("Couldn't parse: {}", msg))]
    ParseError { msg: String },
}

pub type CliResult<T, E = CliError> = std::result::Result<T, E>;