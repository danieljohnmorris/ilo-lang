# Idea 3: Constrained Decoding

ilo isn't a language — it's a grammar fed to a constrained decoder. The agent generates tokens, but at each step the runtime masks invalid next-tokens. The agent literally cannot write an invalid program.

The runtime provides a JSON Schema. The constrained decoder (Outlines/Guidance/LMQL) uses it to mask invalid tokens at each generation step.

- Pro: impossible to generate invalid programs (zero retries)
- Pro: agents already know JSON
- Con: schema must be loaded into context (expensive for large systems)
- Con: constrained decoding is slower per token
- Con: flow control in JSON is awkward
