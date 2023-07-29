# hostmonitoring usage instructions

### Running the agent
    # View the agent help message
    ./target/debug/hostmonitoring-agent -h

> usage: hostmonitoring-agent [-h] [--log-root LOG-ROOT] PORT
>
> positional arguments:</br>
> ㅤPORT        The HTTP port to listen on.
>
> options:</br>
> ㅤ-h, --help  Show this help message and exit.</br>
> ㅤ--log-root LOG-ROOT  Path to the logs to expose (default: /var/log).

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
