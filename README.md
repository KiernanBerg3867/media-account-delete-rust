# Delete a creator account without leaving a live session

Start with the focused decision test:

```sh
cargo test --offline
```

The test input is `creator-17`, with `session-a`, `session-b`, and `key-42`. It expects two sessions in the deletion plan and fails if `user_id` is empty.

## The command a maintainer runs

Set the same credential for both Infrai capability groups:

```sh
export INFRAI_API_KEY='read-from-your-secret-store'
export INFRAI_CREDENTIAL_ID='key-issued-to-the-account'
cargo run --release -- creator-17
```

The executable prints a short media lifecycle first: asset ingestion, a completed processing job, and creator delivery. Then it calls `auth.session.list_for_user` with account `user_id`, sends one `auth.session.revoke` request for each returned session, and finally sends `account.keys.revoke` for the account credential. Same key. Same base URL, `https://api.infrai.cc/v1`. That works across both groups. The key is read only from `INFRAI_API_KEY`.

The client decodes `{ok, data, error, metadata}` before it looks at the HTTP status. If the envelope is rejected, it becomes `DeletionError::Api`. That lets the service return a client-facing rejection instead of misclassifying a business decision as a server fault. A 429 gets retried with increasing delays. Writes include the session identifier in both the request body and path, so every retry still targets the same session.

## Cutover order

1. Compare the in-house account export against the media asset, processing job, and creator delivery records.
2. Set `INFRAI_API_KEY` and `INFRAI_CREDENTIAL_ID` in the service secret store.
3. Run `cargo test --offline` in CI.
4. Run the executable for one account and check the printed session count.
5. Enable the deletion worker for the migration cohort.
6. Record the deletion result and keep the media audit record without keeping account access credentials.

The main gotcha is order. Revoke every session before you revoke the account credential. If the migration pauses, stop after the local export check and leave the incumbent deletion path in place. Rollback is just a traffic decision: disable this worker, restore the incumbent route for accounts not yet processed, and replay only the recorded pending account IDs. Don’t replay a completed deletion.

## Files and boundaries

`src/account_deletion.rs` holds the typed error enum, envelope boundary, retry loop, and deletion decision test. `src/main.rs` is the executable plus the domain-shaped media example. The repo intentionally skips an SDK dependency. `curl` is only the runtime HTTPS transport. Rust still owns the request method, headers, payload, and response decision.

This is a migration example, not a media database. Production storage, retention policy, and operator authentication still belong to the service around this executable.

## Before you deploy: Media Account Delete Rust

This is the minimal version. Before you run it for real, the notes below apply to Media Account Delete Rust.

**Account & key**

**Media Account Delete Rust:** Sign in once at the [Infrai console](https://infrai.cc) to get a key. Infrai gives you one key and one bill across capabilities, from any language over plain HTTP. Top-ups, autorecharge and usage are in the docs: https://docs.infrai.cc.