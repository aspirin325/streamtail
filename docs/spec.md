# streamtail

A CLI utility that continuously follows changing web content,
similar to `tail -f` but for URLs and HTTP streams.

## Goals

- Follow changing HTTP responses
- Stream incremental updates
- Support polling and streaming protocols
- Work well in terminals and pipes
- Minimal dependencies
- Fast startup

## Supported and planned sources

- Plain HTTP and HTTPS endpoints: Phase 1
- SSE streams: Phase 2
- NDJSON: Phase 2
- Chunked transfer encoding: Phase 2
- WebSocket: Phase 3

## Example usage

```sh
streamtail https://example.com/logs

streamtail https://example.com/api/events \
  --interval 2s

streamtail https://example.com/stream \
  --json
```

## Core behavior

- Detect appended content
- Print only new lines by default
- Preserve ordering
- Recover from disconnects
- Retry with exponential backoff
- Treat 5xx and transport failures as retryable
- Treat 401 and 403 responses as fatal

## Non-goals

- Full browser rendering
- JavaScript execution
- Headless Chromium integration

## Phase 1 implementation scope

- Blocking HTTP/HTTPS polling with request timeouts
- Append-only diffing between response snapshots
- Plain text output by default
- JSON Lines output with `--json`
- Manual CLI parser to keep dependencies minimal
- SemVer package versions and release tags in `vMAJOR.MINOR.PATCH` format
