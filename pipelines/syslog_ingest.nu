# Syslog ingestion pipeline
# Receives raw log lines from SyslogWatcher, parses them, and stores as events.

let line = $args[0];
let parts = line.split("\t");
let source = parts[0];
let content = parts[1];

# Parse syslog format: "Sep 22 14:39:54 [kernel] message"
# Extract timestamp, facility, and message

# Store as event in mirror-log
exec("mirror-log ingest --source syslog --data '" + source + "' --content '" + content + "'");