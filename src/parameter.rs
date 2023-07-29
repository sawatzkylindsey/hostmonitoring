use blarg::{CommandLineParser, GeneralParser, Parameter, Scalar};
use std::path::PathBuf;

/// The default log_root.
const DEFAULT_LOG_ROOT: &str = "/var/log";

/// Container for the parameters to extract from the Cli invocation.
#[derive(Debug, PartialEq, Eq)]
pub struct Parameters {
    /// The port for the HTTP server to listen on.
    pub port: u16,
    /// The optional root for the log files to provide access to.
    /// Defaults to `DEFAULT_LOG_ROOT`.
    pub log_root: PathBuf,
}

/// Parse the Cli inputs based off the system args.
pub fn parse() -> Parameters {
    parse_tokens(|parser: GeneralParser| Ok(parser.parse()))
}

/// Parse arbitrary input args.
/// We implement this separate from `parse` for unit testing purposes.
///
/// This function is private/local to this file scope only.
fn parse_tokens(parse_fn: impl FnOnce(GeneralParser) -> Result<(), i32>) -> Parameters {
    let mut parameters = Parameters {
        port: 0,
        log_root: DEFAULT_LOG_ROOT.into(),
    };

    let clp = CommandLineParser::new("hostmonitoring-agent");
    let parser = clp
        .add(
            // The port is a required positional parameter.
            Parameter::argument(Scalar::new(&mut parameters.port), "port")
                .help("The HTTP port to listen on."),
        )
        .add(
            // The log_root is an optional parameter, specified via `--log-root /some/path`.
            Parameter::option(Scalar::new(&mut parameters.log_root), "log-root", None)
                .help("Path to the logs to expose (default: /var/log)."),
        )
        .build()
        .expect("Invalid argument parser configuration");
    // We use an expect for the result of `parse_fn`, since it can only fail in test.
    // If the actual runtime invocation fails, then the Cli library `blarg` will use system exit before we get here.
    parse_fn(parser).expect("parse_fn may only fail in test");
    parameters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        // Setup
        let tokens = vec!["123"];

        // Execute
        let result = parse_tokens(|parser| parser.parse_tokens(tokens.as_slice()));

        // Verify
        assert_eq!(
            result,
            Parameters {
                port: 123,
                log_root: DEFAULT_LOG_ROOT.into()
            }
        );
    }

    #[test]
    fn parse_log_root() {
        // Setup
        let tokens = vec!["123", "--log-root", "abc/123"];

        // Execute
        let result = parse_tokens(|parser| parser.parse_tokens(tokens.as_slice()));

        // Verify
        assert_eq!(
            result,
            Parameters {
                port: 123,
                log_root: "abc/123".into()
            }
        );
    }
}
