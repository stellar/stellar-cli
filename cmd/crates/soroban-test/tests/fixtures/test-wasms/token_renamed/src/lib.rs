#![no_std]
//! A minimal SEP-41-shaped token whose function *parameters* deliberately use
//! non-canonical names — `balance(who)`, `transfer(sender, recipient, amt)`,
//! `decimals()` — instead of SEP-41's `id`/`from`/`to`/`amount`. It exists so
//! integration tests can prove `stellar token` maps values to the contract's
//! parameters by position, not by name.
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Map, Symbol};

const BALANCES: Symbol = symbol_short!("BAL");
const DECIMALS: Symbol = symbol_short!("DEC");

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    /// Set the token's decimals. Not part of SEP-41; test setup only.
    pub fn init(env: Env, decimal_count: u32) {
        env.storage().instance().set(&DECIMALS, &decimal_count);
    }

    /// Seed a balance. Not part of SEP-41; test setup only.
    pub fn mint(env: Env, dest: Address, qty: i128) {
        let mut balances = Self::balances(&env);
        let current = balances.get(dest.clone()).unwrap_or(0);
        balances.set(dest, current + qty);
        env.storage().instance().set(&BALANCES, &balances);
    }

    /// SEP-41 `balance(id) -> i128`, with the parameter renamed to `who`.
    pub fn balance(env: Env, who: Address) -> i128 {
        Self::balances(&env).get(who).unwrap_or(0)
    }

    /// SEP-41 `decimals() -> u32`.
    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&DECIMALS).unwrap_or(0)
    }

    /// SEP-41 `transfer(from, to, amount)`, with the parameters renamed to
    /// `sender`/`recipient`/`amt`. `sender` authorizes the move.
    pub fn transfer(env: Env, sender: Address, recipient: Address, amt: i128) {
        sender.require_auth();
        let mut balances = Self::balances(&env);
        let from_bal = balances.get(sender.clone()).unwrap_or(0);
        balances.set(sender, from_bal - amt);
        let to_bal = balances.get(recipient.clone()).unwrap_or(0);
        balances.set(recipient, to_bal + amt);
        env.storage().instance().set(&BALANCES, &balances);
    }

    fn balances(env: &Env) -> Map<Address, i128> {
        env.storage()
            .instance()
            .get(&BALANCES)
            .unwrap_or_else(|| Map::new(env))
    }
}
