use near_sdk::{AccountId, env, PromiseResult, require};
use crate::MIN_PLAYER_COST;

pub fn create_new_account_id(game_id: u32, base_id: &AccountId) -> AccountId {
    format!("game_{game_id}.{base_id}").parse().unwrap()
}

pub fn is_promise_success() -> bool {
    assert_eq!(env::promise_results_count(), 1, "Contract expected a result on the callback");
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

pub fn assert_deposit() {
    require!(env::attached_deposit() >= MIN_PLAYER_COST,
        "Attached deposit must be greater than MIN_PLAYER_FEE");
}