use near_sdk::{collections::UnorderedMap, json_types::U128, AccountId};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{serde::Serialize, serde::Deserialize};
use near_sdk::{near_bindgen, env, PanicOnDefault, Promise, Gas};
pub mod utils;

const XCC_GAS: Gas = Gas(5 * 10u64.pow(13));

#[derive(Serialize, Deserialize, BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum Choice {
    Rock,
    Paper,
    Scissors
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
struct FactoryContractOnWinnerArgs {
    winner: Option<AccountId>,
}

pub enum StorageKeys {
    CommitmentKey,
    ChoiceKey
}

// this contract is gonna be spawn by a player who is gonna wait for an player2
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    player1: AccountId,
    player2: AccountId,
    factory: AccountId,
    commitments: UnorderedMap<AccountId, String>,
    choices: UnorderedMap<AccountId, Choice>,
}

// Implement the contract structure
#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(player1: AccountId, player2: AccountId, factory: AccountId) -> Self {
        Contract {
            player1,
            player2,
            factory,
            commitments: UnorderedMap::new(StorageKeys::CommitmentKey as u8),
            choices: UnorderedMap::new(StorageKeys::ChoiceKey as u8),
        }
    }

    pub fn get_commitment_hash(&self, choice: Choice, secret: U128) -> String {
        utils::assert_choice(choice);

        let da_secret: u128 = secret.into();
        let mut concat: Vec<u8> = vec![choice as u8];
        concat.append(&mut da_secret.to_be_bytes().to_vec());
        hex::encode(env::keccak256(&concat))
    }

    pub fn make_commitment(&mut self, commitment_hash: String) {
        utils::assert_players(&self.player1, &self.player2);

        self.commitments.insert(&env::signer_account_id(), &commitment_hash);
    }

    pub fn reveal_commitment(&mut self, choice: Choice, secret: U128) {
        utils::assert_choice(choice);
        utils::assert_players(&self.player1, &self.player2);
        utils::assert_commitment(&hex::encode(self.get_commitment_hash(choice, secret)),
                                 &self.commitments.get(&env::signer_account_id()).unwrap());

        self.choices.insert(&env::signer_account_id(), &choice);

        // choice.len() cannot be larger than 2, but in case there is an unseen error
        // this will just to make sure the contract does not get stucked
        if self.choices.len() >= 2 {
            self.get_winner();
        }
    }

    #[private]
    fn get_winner(&self) {
        let choice1 = self.choices.get(&self.player1).unwrap();
        let choice2 = self.choices.get(&self.player2).unwrap();

        let mut winner: Option<AccountId> = None;

        if choice1 != choice2 {
            winner = Some(self.player1.clone());

            if choice2 == Choice::Rock && choice1 == Choice::Scissors {
                winner = Some(self.player2.clone());
            }
            else if choice2 == Choice::Paper && choice1 == Choice::Rock {
                winner = Some(self.player2.clone());
            }
            else if choice2 == Choice::Scissors && choice1 == Choice::Paper {
                winner = Some(self.player2.clone());
            }
        }
        
        let args = near_sdk::serde_json::to_vec(&FactoryContractOnWinnerArgs{winner}).unwrap();
        Promise::new(self.factory.clone()).function_call("on_game_finished".to_owned(), args, 0, XCC_GAS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, VMContext};
    use std::str::FromStr;

    fn alice() -> AccountId {
        "alice".parse().unwrap()
    }

    fn bob() -> AccountId {
        "bob".parse().unwrap()
    }

    fn factory() -> AccountId {
        "factory".parse().unwrap()
    }

    fn til() -> AccountId {
        "til".parse().unwrap()
    }

    // part of writing unit tests is setting up a mock context
    fn get_context(acct: AccountId) -> VMContext {
        let mut builder = VMContextBuilder::new();
        builder.current_account_id(AccountId::from_str("game.near").unwrap());
        builder.signer_account_id(acct);
        builder.build()
    }

    #[test]
    #[should_panic]
    fn test_make_commitment_hash() {
        testing_env!(get_context(factory()));
        let mut contract = Contract::new(bob(), alice(), factory());

        testing_env!(get_context(til()));
        let commitment_hash = contract.get_commitment_hash(Choice::Paper, U128(10u128));
        contract.make_commitment(commitment_hash);
    }
    
    #[test]
    #[should_panic]
    fn test_reveal_commitment_bad_secret() {
        testing_env!(get_context(factory()));
        let mut contract = Contract::new(bob(), alice(), factory());

        testing_env!(get_context(bob()));
        let commitment_hash = contract.get_commitment_hash(Choice::Paper, U128(10u128));
        contract.make_commitment(commitment_hash);
        contract.reveal_commitment(Choice::Paper, U128(100u128));
    }

    
    #[test]
    #[should_panic]
    fn test_reveal_commitment_bad_choice() {
        testing_env!(get_context(factory()));
        let mut contract = Contract::new(bob(), alice(), factory());

        testing_env!(get_context(bob()));
        let commitment_hash = contract.get_commitment_hash(Choice::Paper, U128(10u128));
        contract.make_commitment(commitment_hash);
        contract.reveal_commitment(Choice::Rock, U128(10u128));
    }
}