# Expiring capability tokens

Access control now supports additive, single-action capability tokens. An
admin issues a token to one grantee with an action symbol, bounded lifetime,
and use budget. The grantee must authorize `consume_capability`; consumption
checks the action, grantee, expiry, revocation flag, and remaining uses before
incrementing the counter. The issuer or an admin can revoke a token after a
leak. Existing role checks remain in place, so removing this feature does not
change existing authorization paths.
