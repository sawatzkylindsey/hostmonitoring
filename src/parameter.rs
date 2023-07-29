use blarg::{CommandLineParser, GeneralParser, Parameter, Scalar};

/// Container for the parameters to extract from the Cli invocation.
#[derive(Debug, PartialEq, Eq)]
pub struct Parameters {
    pub port: u16,
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
    let mut parameters = Parameters { port: 0 };

    let clp = CommandLineParser::new("hostmonitoring-agent");
    let parser = clp
        .add(
            Parameter::argument(Scalar::new(&mut parameters.port), "port")
                .help("The HTTP port to listen on."),
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
    fn parse_parameters() {
        // Setup
        let tokens = vec!["123"];

        // Execute
        let result = parse_tokens(|parser| parser.parse_tokens(tokens.as_slice()));

        // Verify
        assert_eq!(result, Parameters { port: 123 });
    }
}
