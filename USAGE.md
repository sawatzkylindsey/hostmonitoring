# hostmonitoring usage instructions

### Running the agent
    # View the agent help message
    ./target/debug/hostmonitoring-agent -h

> usage: hostmonitoring-agent [-h] PORT
>
> positional arguments:</br>
> PORT        The HTTP port to listen on.
>
> options:</br>
> -h, --help  Show this help message and exit.

    # Run the agent server
    ./target/debug/hostmonitoring-agent 123
    
    .. runs indefinitely, exit with CTRL+C ..

### Querying the agent

    curl http://localhost:8081/inspect/dir/path

> ["pretend1","pretend2"]

    curl http://localhost:8081/noop

> ["pretend1","pretend2"]

    curl -f http://localhost:8081/noop

> curl: (22) The requested URL returned error: 404

    curl -f http://localhost:8081/

> curl: (22) The requested URL returned error: 404
