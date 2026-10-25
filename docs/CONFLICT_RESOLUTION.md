# Deterministic conflict resolution

`shared::resolve_conflict_pair` resolves two concurrent client edits using a
stable ordering: the greatest revision wins, and equal revisions are ordered
by the SHA-256 digest of the payload. The result includes the chosen revision
and digest so clients can persist an audit decision and repeat retries without
depending on request arrival order. The resolver does not overwrite storage;
callers remain responsible for authorization and applying the selected edit.
