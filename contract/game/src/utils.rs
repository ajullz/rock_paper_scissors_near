use near_sdk::{env, Balance, require, AccountId};
use crate::Choice;

pub fn assert_minimum_fee(fee: Balance) {
    require!(env::attached_deposit() == fee, "Requires attached deposit of exactly -- NEAR tokens");
}

pub fn assert_choice(choice: Choice) {
    require!(choice as u8 <= 2, "Provided choice is not valid!");
}

pub fn assert_players(player1: &AccountId, player2: &AccountId) {
    require!(player1.as_ref() != "" && player2.as_ref() != "", "Missing players");
    require!(env::signer_account_id() == *player1 || env::signer_account_id() == *player2, 
    "You are not allow to participate in this game!");
}

pub fn assert_commitment(revealed: &String, committed: &String) {
    require!(revealed == committed, "This is not the same as your previous commitment.");
}