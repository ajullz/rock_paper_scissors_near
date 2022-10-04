use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{near_bindgen, env, ONE_NEAR, Balance, PanicOnDefault};
use near_sdk::{collections::UnorderedMap, collections::Vector, AccountId};
use near_sdk::{serde::Serialize, Gas, Promise, log, require};
pub mod utils;

const GAME_FEE_FACTOR: f32 = 0.95;
const MIN_STORAGE_COST: Balance = ONE_NEAR / 10;
const MIN_PLAYER_COST: Balance = ONE_NEAR / 2;
const XCC_GAS: Gas = Gas(5 * 10u64.pow(13));

// is this the right way to include other contracts? 
const GAME_CONTRACT: &[u8] = include_bytes!("../../target/wasm32-unknown-unknown/release/game.wasm");

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
struct GameContractInitArgs {
    player1: AccountId,
    player2: AccountId,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct WaitingPlayer {
    account: AccountId,
    deposit: Balance,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    game_instances: UnorderedMap<AccountId, Balance>, // <game instance account, winner pot>
    waiting_list: Vector<WaitingPlayer>,
    next_game_id: u32,
}

pub enum StorageKeys {
    GameInstanceKey,
    WaitingListKey,
}

// Implement the contract structure
#[near_bindgen]
impl Contract {
    #[init]
    pub fn new() -> Self {
        Contract {
            game_instances: UnorderedMap::new(StorageKeys::GameInstanceKey as u8),
            waiting_list: Vector::new(StorageKeys::WaitingListKey as u8),
            next_game_id: 0,
        }
    }

    // each player needs to first wait for an opponent
    // they need to register by attaching a deposit of at least MIN_PLAYER_COST
    // one the waiting list has at least 2 players, it will start a new game round
    #[payable]
    pub fn enter_waiting_list(&mut self) {
        utils::assert_deposit();

        self.waiting_list.push(&WaitingPlayer{account: env::signer_account_id(),
                                              deposit: env::attached_deposit()});

        if self.waiting_list.len() >= 2 {

            // Should I do this in a cross call function to myself to reduce the gas costs on the user?
            // Does that even reduce the costs on the user?
            self.create_new_game_instances();
        }
    }

    // shall only called by the sub account {game_instance}.current_account_id
    pub fn on_game_finished(&mut self, winner_account: Option<AccountId>) {
        let game_account = &env::predecessor_account_id();
        let amount = self.game_instances.get(game_account);
        require!(amount != None, "Only a valid game instance account can call this function");

        // winner_account is of type Option because there can be a draw, 
        // in which case the game wins and keeps the deposits
        if let Some(winner) = winner_account {
            Promise::new(winner).transfer(amount.unwrap());
        }

        Promise::new(game_account.clone())
            .delete_account(env::current_account_id()) 
            .then(Self::ext(env::current_account_id()).on_game_contract_deleted(game_account));
    }

    #[private]
    fn create_new_game_instances(&mut self) {
        let waiting_list_len = self.waiting_list.len();

        for i in (0..waiting_list_len).step_by(2) {
            if (i + 1) < waiting_list_len {                
                let player1 = self.waiting_list.pop().unwrap();
                let player2 = self.waiting_list.pop().unwrap();

                let deposit1 = player1.deposit;
                let deposit2 = player2.deposit;
                let deposit = std::cmp::min(deposit1, deposit2);
                
                // if one of the players deposited more, transfer back the difference
                if deposit1 > deposit2 {
                    Promise::new(player1.account.clone()).transfer((deposit1 - deposit2).into());
                }
                else if deposit1 < deposit2 {
                    Promise::new(player2.account.clone()).transfer((deposit2 - deposit1).into());
                }

                self.create_new_game_instance(player1.account, player2.account, deposit.into());
            }
        }
    }

    #[private]
    fn create_new_game_instance(&mut self, player1: AccountId, player2: AccountId, deposit: Balance) {
        let game_account = utils::create_new_account_id(self.next_game_id, &env::current_account_id());
        let init_args = near_sdk::serde_json::to_vec(&GameContractInitArgs{
            player1: player1.clone(), 
            player2: player2.clone()})
            .unwrap();

        self.next_game_id += 1;

        Promise::new(game_account.clone())
            .create_account()
            .transfer(MIN_STORAGE_COST)
            .deploy_contract(GAME_CONTRACT.to_vec())
            .function_call("new".to_owned(), init_args, 0, XCC_GAS)
            .then(Self::ext(env::current_account_id()).on_game_contract_deployed(&game_account, deposit, player1, player2)
        );
    }

    // if the game contract could be deployed, we update the game_instances map
    // otherwise, we send the funds back to each player
    #[private]
    pub fn on_game_contract_deployed(&mut self, game_account: &AccountId, deposit: Balance, player1: AccountId, player2: AccountId) {

        if utils::is_promise_success() {
            log!("Successfully deployed game contract to {game_account}");
            let game_deposit = ((2.0 * deposit as f32) * GAME_FEE_FACTOR) as u128;
            self.game_instances.insert(game_account, &game_deposit);
        }
        else {
            log!("Error while deploying game contract to {game_account}. Funds will be sent back.");
            Promise::new(player1).transfer(deposit);
            Promise::new(player2).transfer(deposit);
        }
    }

    // no matter what happens when deleting the sub account (where the game contract is deployed to), 
    // it will always remove the game account id from the game_instances map
    #[private]
    pub fn on_game_contract_deleted(&mut self, game_account: &AccountId) {

        if utils::is_promise_success() {
            log!("Successfully deleted game contract to {game_account}");
        }
        else {
            log!("Error while deleting game contract to {game_account}. Game instance will still be deleted");
        }
        
        self.game_instances.remove(game_account);
    }
}


#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use near_sdk::test_utils::{VMContextBuilder};
    use near_sdk::{testing_env};
    use super::*;

    fn alice() -> AccountId {
        "alice".parse().unwrap()
    }

    fn bob() -> AccountId {
        "bob".parse().unwrap()
    }

    #[test]
    #[should_panic]
    fn test_enter_waiting_list_bad_deposit() {
        let mut factory = Contract::new();
        let deposit = MIN_PLAYER_COST / 2;
        
        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
            .current_account_id(alice())
            .attached_deposit(deposit)
            .context.clone()
        );

        // Create bob's account with the PK
        factory.enter_waiting_list();
    }

    #[test]
    #[should_panic]
    fn test_on_game_finished_bad_caller() {
        let mut factory = Contract::new();
        let deposit = MIN_PLAYER_COST;
        
        // create context for alice
        testing_env!(
            VMContextBuilder::new()
            .current_account_id(alice())
            .attached_deposit(deposit)
            .context.clone()
        );

        // add alice to the waiting list
        factory.enter_waiting_list();

        // create context for bob
        testing_env!(
            VMContextBuilder::new()
            .current_account_id(bob())
            .attached_deposit(deposit)
            .context.clone()
        );

        // add bob to the waiting list
        factory.enter_waiting_list();

        // this should panic
        factory.on_game_finished(Some(bob()));
    }
}