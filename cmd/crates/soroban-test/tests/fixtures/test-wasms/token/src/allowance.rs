use crate::storage_types::{AllowanceDataKey, AllowanceValue, DataKey};
use soroban_sdk::{Address, Env};

pub fn read(e: &Env, from: Address, spender: Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceDataKey { from, spender });
    if let Some(allowance) = e.storage().temporary().get::<_, AllowanceValue>(&key) {
        if allowance.expiration_ledger < e.ledger().sequence() {
            AllowanceValue {
                amount: 0,
                expiration_ledger: allowance.expiration_ledger,
            }
        } else {
            allowance
        }
    } else {
        AllowanceValue {
            amount: 0,
            expiration_ledger: 0,
        }
    }
}

pub fn write(e: &Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
    let allowance = AllowanceValue {
        amount,
        expiration_ledger,
    };

    assert!(
        !(amount > 0 && expiration_ledger < e.ledger().sequence()),
        "expiration_ledger is less than ledger seq when amount > 0"
    );

    let key = DataKey::Allowance(AllowanceDataKey { from, spender });
    e.storage().temporary().set(&key.clone(), &allowance);

    if amount > 0 {
        let live_for = expiration_ledger
            .checked_sub(e.ledger().sequence())
            .unwrap();

        e.storage().temporary().extend_ttl(&key, live_for, live_for);
    }
}

pub fn spend(e: &Env, from: Address, spender: Address, amount: i128) {
    let allowance = read(e, from.clone(), spender.clone());
    assert!(allowance.amount >= amount, "insufficient allowance");
    write(
        e,
        from,
        spender,
        allowance.amount - amount,
        allowance.expiration_ledger,
    );
}
