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

> ["", "3 abcdef", "2 def", "1 abc"]

    curl http://localhost:8081/inspect/long.log

> ["99999", "99998", ..

    curl http://localhost:8081/inspect/long.log?substring[]=123

> ["99123", "98123", ..

    # This file is ~1921 MB.
    # Takes about 1.5 minutes & less than 8 MB on the hostmonitoring-agent on my computer.
    curl http://localhost:8081/inspect/large.log -O
    cat large.log | jq ". | length"

> 100000

    curl http://localhost:8081/inspect/large.log?limit=100 -O
    cat large.log | jq ". | length"

> 100

    curl -f http://localhost:8081/inspect/noop

> curl: (22) The requested URL returned error: 404

    curl -f http://localhost:8081/noop

> curl: (22) The requested URL returned error: 404

    curl -f http://localhost:8081/

> curl: (22) The requested URL returned error: 404

    curl -f http://localhost:8081/inspect/../Cargo.toml

> curl: (22) The requested URL returned error: 404

### Frequently Asked Questions (FAQ)
* The agent keeps returning `400`, but I'm querying for actual files (they exist).

> The hostmonitoring-agent must be configured to run against a canonical/absolute file path.
> Double check how your file system structures the log root.
> For example, sometimes `/var/log` is actually symbolically linked from `/private`.
> In this case, the canonical log root is `/private/var/log` (the agent won't discover this for you).
