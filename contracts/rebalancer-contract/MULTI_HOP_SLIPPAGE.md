# Multi-hop slippage circuit breaker

`execute_multi_hop` is the guard at the boundary between a route planner and
the downstream swap venues. The configured admin must authorize the call.
Before any venue call is made, the guard verifies that:

1. the route has at least one hop and every hop changes assets;
2. quote and observed-output vectors match the route and contain positive values;
3. every observed output is no greater than its quote and the cumulative
   per-hop slippage stays within `max_slippage_bps`; and
4. the final observed output meets `min_final_output`.

The check has no storage side effects. A failed quote can therefore be retried
with a fresh receipt without consuming route state. Callers should discard the
failed quote and obtain new quotes before retrying. The existing single-hop
`rebalance` entry point is unchanged, so deployments can adopt this guard
without a migration.
