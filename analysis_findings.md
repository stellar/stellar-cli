## Repository Analysis for Issue #1019

### Key Findings:

1. **Current State**: The `simulate` command in `soroban-cli/src/commands/simulate.rs` returns `SimCost` with only `cpu_instructions` and `mem` fields. It doesn't expose other fee/budget metrics like:
   - Read bytes
   - Write bytes
   - Historical fee
   - Transaction footprints

2. **Missing in Error Cases**: When simulation fails, no budget information is returned at all, making it impossible to diagnose which limit was exceeded.

3. **Relevant Files**:
   - `soroban-cli/src/commands/simulate.rs` - Main simulation command
   - `soroban-cli/src/output.rs` - Output formatting
   - `soroban-core/src/simulation.rs` - Simulation logic
   - `soroban-sdk/src/transaction.rs` - Transaction structures

### Required Changes:
1. Expand `SimCost` to include ALL cost metrics from Soroban simulation results
2. Return budget info even on simulation failure
3. Update output formatting to display all metrics
