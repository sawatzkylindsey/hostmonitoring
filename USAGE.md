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
    ./target/debug/hostmonitoring-agent 123 --log-root /Users/me/hostmonitoring/test-data
    
    .. runs indefinitely, exit with CTRL+C ..

### Querying the agent

    curl http://localhost:8081/inspect/service.log

> ["", "2 def", "1 abc"]

    curl http://localhost:8081/inspect/long.log

> ["99999", "99998", ..

    # This file is ~1921 MB.
    # Takes about 1.5 minutes & less than 8 MB on the hostmonitoring-agent on my computer.
    curl http://localhost:8081/inspect/large.log -O
    cat large.log | jq ". | length"

> 100000

    curl -f http://localhost:8081/inspect/noop

> curl: (22) The requested URL returned error: 404

    curl -f http://localhost:8081/noop

> curl: (22) The requested URL returned error: 404

    curl -f http://localhost:8081/

> curl: (22) The requested URL returned error: 404
