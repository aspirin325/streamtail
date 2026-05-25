# Testing

## Incremental output

Given:
hello

Then:
hello world

Only:
 world

Should be emitted.

## Reconnect behavior

Should retry:
- network failures
- 5xx responses
- statuses passed with `--retry-status`

Should not retry:
- 401
- 403

## Color output

`--color` should add ANSI color codes only to text output written to standard
output. JSON Lines output and files selected with `--output` should remain
plain and parseable.

When multiple URLs are provided, each URL should keep an independent diff
snapshot. Text output to standard output should use different automatic colors
per URL unless `--color` overrides the next URL or `--no-color` disables all
color formatting.

## Follow from end

Given the first response:
hello

And the next response:
hello world

With `--follow-from-end`, only:
 world

Should be emitted.
