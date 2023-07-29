# hostmonitoring
Demonstration project for host inspection.

### Problem
We need to provide on-demand monitoring for arbitrary unix hosts in a fleet.
The key use case is to view `/var/log` without logging onto the host directly.
We've been tasked specifically to provide this access via REST Api.

#### Requirements
Let's make a few assumptions at the outset:

* The "inspector" knows the file (and path) they want to view a-priori.
That is, we needn't provide support for finding/listing files.
* The "inspector" wants to view a single file a time.
Separate files may be viewed by distinct invocations of our solution.
* The "inspector" views log lines via HTTP REST Api.
We should only provide access to files within `/var/log`.

From here, let's write down our requirements:

* Log lines should be returned in reverse chronological order (newest logs first, oldest logs last).
* Support for the following parameters:
    * (required) The filename within `/var/log`.
    * (optional) Filter for log lines by substring match.
    * (optional) Limit the lines by some number `N`.
* Low CPU/memory impact to the host (the host must be able to continue its regular work).
* Must be able to read large log files.
* Key logic implemented directly in the project itself (aka: don't use an existing host monitoring tool/library).
* Files that don't exist in `/var/log` should return an appropriate error (ex: HTTP `404`).
* Only file + filepaths that form a valid absolute path inside `/var/log` should be accessible.

Given these requirements are satisfied, we may consider the some future requirements/enhancements.
It may be useful to keep these in mind for the design.

* Provide a CLI/UI client.
* Support inspecting the logs across multiple hosts, via a single host.

