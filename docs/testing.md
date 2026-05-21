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

## Follow from end

Given the first response:
hello

And the next response:
hello world

With `--follow-from-end`, only:
 world

Should be emitted.
