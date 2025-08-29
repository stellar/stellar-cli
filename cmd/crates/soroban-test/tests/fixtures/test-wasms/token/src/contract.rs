//! This contract demonstrates a sample implementation of the Soroban token
//! interface.
use crate::storage_types::{INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};
use crate::{admin, allowance, balance, metadata};
use soroban_sdk::token::{self, Interface as _};
use soroban_sdk::{contract, contractevent, contractimpl, Address, Env, MuxedAddress, String};
use soroban_token_sdk::metadata::TokenMetadata;
use soroban_token_sdk::events::{TransferWithAmountOnly, Approve, Burn, MintWithAmountOnly};

#[contractevent(data_format = "single-value")]
pub struct SetAdmin {
    #[topic]
    admin: Address,
    new_admin: Address,
}

fn check_nonnegative_amount(amount: i128) {
    assert!(amount >= 0, "negative amount is not allowed: {amount}");
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        assert!(!admin::has(&e), "already initialized");
        admin::write_administrator(&e, &admin);
        assert!(decimal <= u8::MAX.into(), "Decimal must fit in a u8");

        metadata::write(
            &e,
            TokenMetadata {
                decimal,
                name,
                symbol,
            },
        );
    }

    pub fn mint(e: Env, to: Address, amount: i128) {
        check_nonnegative_amount(amount);
        let admin = admin::read_administrator(&e);
        admin.require_auth();

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        balance::receive(&e, to.clone(), amount);
        MintWithAmountOnly {
            to: to.clone(),
            amount,
        }.publish(&e);
    }

    pub fn set_admin(e: Env, new_admin: Address) {
        let admin = admin::read_administrator(&e);
        admin.require_auth();

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        admin::write_administrator(&e, &new_admin);
        SetAdmin {
            admin: admin.clone(),
            new_admin: new_admin.clone(),
        }.publish(&e);
    }
}

#[contractimpl]
impl token::Interface for Token {
    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
        allowance::read(&e, from, spender).amount
    }

    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        allowance::write(&e, from.clone(), spender.clone(), amount, expiration_ledger);
        Approve {
            from: from.clone(),
            spender: spender.clone(),
            amount,
            expiration_ledger,
        }.publish(&e);
    }

    fn balance(e: Env, id: Address) -> i128 {
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
        balance::read(&e, id)
    }

    fn transfer(e: Env, from: Address, to: MuxedAddress, amount: i128) {
        from.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        let to_address = to.address();
        balance::spend(&e, from.clone(), amount);
        balance::receive(&e, to_address.clone(), amount);
        TransferWithAmountOnly {
            from: from.clone(),
            to: to_address,
            amount,
        }.publish(&e);
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        allowance::spend(&e, from.clone(), spender, amount);
        balance::spend(&e, from.clone(), amount);
        balance::receive(&e, to.clone(), amount);
        TransferWithAmountOnly {
            from: from.clone(),
            to: to.clone(),
            amount,
        }.publish(&e);
    }

    fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        balance::spend(&e, from.clone(), amount);
        Burn {
            from: from.clone(),
            amount,
        }.publish(&e);
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        allowance::spend(&e, from.clone(), spender, amount);
        balance::spend(&e, from.clone(), amount);
        Burn {
            from: from.clone(),
            amount,
        }.publish(&e);
    }

    fn decimals(e: Env) -> u32 {
        metadata::decimal(&e)
    }

    fn name(e: Env) -> String {
        metadata::name(&e)
    }

    fn symbol(e: Env) -> String {
        metadata::symbol(&e)
    }
}
