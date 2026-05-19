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

Should not retry:
- 401
- 403
