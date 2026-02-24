# Idea 5: Workflow DAG

ilo as a YAML workflow definition, like AWS Step Functions or Temporal.

- Named steps make the DAG explicit
- `$` references for data flow between steps
- `catch` blocks for error handling
- `compensate` for rollback
- YAML is well-tokenised (agents generate lots of YAML)
- The runtime walks the DAG, calling tools at each step
- Verification: all tool names checked, all references resolved
