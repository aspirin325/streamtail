# AGENTS.md

## Rules

- Never introduce unnecessary dependencies
- Prefer standard library solutions
- Keep functions under 50 lines when possible
- All network operations must support timeout
- All public APIs require tests
- Use structured errors
- No panic/unwrap in production code

## Style

- Prefer composition over inheritance
- Prefer immutable data
- Avoid global state

## Testing

- Unit tests required for parsers
- Integration tests required for HTTP behavior