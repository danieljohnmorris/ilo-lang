# Idea 2: Extended Tool Calling

ilo as a sequence of tool calls with flow control, built on top of the function-calling JSON format agents already generate.

- `$references` for data flow between steps
- `on-error` for error handling
- `compensate` for rollback on failure
- Runtime is a step executor

Agents already generate JSON for tool calls. This extends that with sequencing and error handling. No new syntax to learn — just JSON with conventions.
