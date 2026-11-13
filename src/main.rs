mod account_deletion;

use account_deletion::{deletion_plan, InfraiClient};

#[derive(Debug)]
struct MediaAsset { id: &'static str, owner: &'static str, state: &'static str }

#[derive(Debug)]
struct ProcessingJob { asset_id: &'static str, state: &'static str }

#[derive(Debug)]
struct CreatorDelivery { asset_id: &'static str, destination: &'static str, state: &'static str }

fn main() {
    let user_id = std::env::args().nth(1).unwrap_or_else(|| "creator-demo".into());
    let credential_id = std::env::var("INFRAI_CREDENTIAL_ID").unwrap_or_else(|_| "temporary-key-id".into());
    let asset = MediaAsset { id: "asset-17", owner: "creator-demo", state: "ingested" };
    let job = ProcessingJob { asset_id: asset.id, state: "complete" };
    let delivery = CreatorDelivery { asset_id: job.asset_id, destination: "creator-library", state: "delivered" };
    println!("media: {:?}; job: {:?}; delivery: {:?}", asset, job, delivery);

    let client = match InfraiClient::from_env() {
        Ok(client) => client,
        Err(error) => { eprintln!("{error}"); std::process::exit(2); }
    };
    let sessions = match client.revoke_sessions(&user_id) {
        Ok(ids) => ids,
        Err(error) => { eprintln!("session revoke failed: {error}"); std::process::exit(1); }
    };
    let plan = deletion_plan(&user_id, sessions.clone(), &credential_id).expect("validated user id");
    if let Err(error) = client.revoke_credential(&plan.credential_id) {
        eprintln!("credential revoke failed: {error}"); std::process::exit(1);
    }
    println!("deleted account {}: revoked {} sessions and credential {}", plan.user_id, plan.session_ids.len(), plan.credential_id);
}
