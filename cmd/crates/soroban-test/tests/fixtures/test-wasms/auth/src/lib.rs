#![no_std]
use soroban_sdk::{contract, contractimpl, vec, Address, Env, IntoVal, Symbol};

#[contract]
pub struct AuthContract;

#[contractimpl]
impl AuthContract {
    /// Constructor with auth
    pub fn __constructor(_env: Env, addr: Address) {
        addr.require_auth();
    }

    /// require_auth on addr
    ///
    /// Used by other functions to emulate different nested auth options
    pub fn do_auth(_e: Env, addr: Address, val: Symbol) -> Symbol {
        addr.require_auth();
        val
    }

    /// require_auth on `addr`
    /// -> `subcall` does require_auth on `addr`
    ///
    /// Used by other functions to emulate different nested auth options
    pub fn auth_sub_auth(e: Env, addr: Address, val: Symbol, subcall: Address) -> Symbol {
        addr.require_auth();

        let fn_symbol = Symbol::new(&e, "do_auth");
        e.invoke_contract::<Symbol>(
            &subcall,
            &fn_symbol,
            vec![&e, addr.into_val(&e), val.into_val(&e)],
        )
    }

    /// require_auth on `addr`
    /// -> `subcall` does require_auth on `addr`
    ///     -> `subcall2` does require_auth on `addr`
    pub fn auth_sub_nested_auth(
        e: Env,
        addr: Address,
        val: Symbol,
        subcall: Address,
        subcall2: Address,
    ) -> Symbol {
        addr.require_auth();

        let fn_symbol = Symbol::new(&e, "auth_sub_auth");
        e.invoke_contract::<Symbol>(
            &subcall,
            &fn_symbol,
            vec![
                &e,
                addr.into_val(&e),
                val.into_val(&e),
                subcall2.into_val(&e),
            ],
        )
    }

    /// require_auth_for_args(val) on `addr`
    /// -> `subcall` does require_auth on `addr`
    pub fn partial_auth_sub_auth(e: Env, addr: Address, val: Symbol, subcall: Address) -> Symbol {
        addr.require_auth_for_args(vec![&e, addr.into_val(&e), val.into_val(&e)]);

        let fn_symbol = Symbol::new(&e, "do_auth");
        e.invoke_contract::<Symbol>(
            &subcall,
            &fn_symbol,
            vec![&e, addr.into_val(&e), val.into_val(&e)],
        )
    }

    /// require_auth_for_args(1i128, 2i128) on `addr`
    /// -> `subcall` does require_auth on `addr`
    pub fn diff_auth_sub_auth(e: Env, addr: Address, val: Symbol, subcall: Address) -> Symbol {
        addr.require_auth_for_args(vec![&e, 1i128.into_val(&e), 2i128.into_val(&e)]);

        let fn_symbol = Symbol::new(&e, "do_auth");
        e.invoke_contract::<Symbol>(
            &subcall,
            &fn_symbol,
            vec![&e, addr.into_val(&e), val.into_val(&e)],
        )
    }

    /// no auth
    /// -> `subcall` does require_auth on `addr`
    pub fn no_auth_sub_auth(e: Env, addr: Address, val: Symbol, subcall: Address) -> Symbol {
        let fn_symbol = Symbol::new(&e, "do_auth");
        e.invoke_contract::<Symbol>(
            &subcall,
            &fn_symbol,
            vec![&e, addr.into_val(&e), val.into_val(&e)],
        )
    }

    /// no auth
    /// -> `subcall` does require_auth on `addr`
    ///     -> `subcall2` does require_auth on `addr`
    pub fn no_auth_sub_nested_auth(
        e: Env,
        addr: Address,
        val: Symbol,
        subcall: Address,
        subcall2: Address,
    ) -> Symbol {
        let fn_symbol = Symbol::new(&e, "auth_sub_auth");
        e.invoke_contract::<Symbol>(
            &subcall,
            &fn_symbol,
            vec![
                &e,
                addr.into_val(&e),
                val.into_val(&e),
                subcall2.into_val(&e),
            ],
        )
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use soroban_sdk::{
        testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
        Address, Env, IntoVal, Symbol,
    };

    use crate::{AuthContract, AuthContractClient};

    #[test]
    fn test_do_auth_creates_expected_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let user = Address::generate(&env);
        let val = Symbol::new(&env, "test_auth");

        let contract_id = env.register(AuthContract, (user.clone(),));
        let client = AuthContractClient::new(&env, &contract_id);

        let res = client.do_auth(&user, &val);
        assert_eq!(
            env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        contract_id.clone(),
                        Symbol::new(&env, "do_auth"),
                        (&user, &val).into_val(&env),
                    )),
                    sub_invocations: std::vec![],
                },
            )]
        );
        assert_eq!(res, val);
    }

    #[test]
    fn test_auth_sub_auth_creates_expected_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let user = Address::generate(&env);
        let val = Symbol::new(&env, "test_auth");

        let contract_id_1 = env.register(AuthContract, (user.clone(),));
        let client_1 = AuthContractClient::new(&env, &contract_id_1);
        let contract_id_2 = env.register(AuthContract, (user.clone(),));

        let res = client_1.auth_sub_auth(&user, &val, &contract_id_2);
        assert_eq!(
            env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        contract_id_1.clone(),
                        Symbol::new(&env, "auth_sub_auth"),
                        (&user, &val, &contract_id_2).into_val(&env),
                    )),
                    sub_invocations: std::vec![AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            contract_id_2.clone(),
                            Symbol::new(&env, "do_auth"),
                            (&user, &val).into_val(&env),
                        )),
                        sub_invocations: std::vec![],
                    }],
                },
            )]
        );
        assert_eq!(res, val);
    }

    #[test]
    fn test_auth_sub_nested_auth_creates_expected_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let user = Address::generate(&env);
        let val = Symbol::new(&env, "test_auth");

        let contract_id_1 = env.register(AuthContract, (user.clone(),));
        let client_1 = AuthContractClient::new(&env, &contract_id_1);
        let contract_id_2 = env.register(AuthContract, (user.clone(),));
        let contract_id_3 = env.register(AuthContract, (user.clone(),));

        let res = client_1.auth_sub_nested_auth(&user, &val, &contract_id_2, &contract_id_3);
        assert_eq!(
            env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        contract_id_1.clone(),
                        Symbol::new(&env, "auth_sub_nested_auth"),
                        (&user, &val, &contract_id_2, &contract_id_3).into_val(&env),
                    )),
                    sub_invocations: std::vec![AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            contract_id_2.clone(),
                            Symbol::new(&env, "auth_sub_auth"),
                            (&user, &val, &contract_id_3).into_val(&env),
                        )),
                        sub_invocations: std::vec![AuthorizedInvocation {
                            function: AuthorizedFunction::Contract((
                                contract_id_3.clone(),
                                Symbol::new(&env, "do_auth"),
                                (&user, &val).into_val(&env),
                            )),
                            sub_invocations: std::vec![],
                        }],
                    }],
                },
            )]
        );
        assert_eq!(res, val);
    }

    #[test]
    fn test_partial_auth_sub_auth_creates_expected_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let user = Address::generate(&env);
        let val = Symbol::new(&env, "test_auth");

        let contract_id_1 = env.register(AuthContract, (user.clone(),));
        let client_1 = AuthContractClient::new(&env, &contract_id_1);
        let contract_id_2 = env.register(AuthContract, (user.clone(),));

        let res = client_1.partial_auth_sub_auth(&user, &val, &contract_id_2);
        assert_eq!(
            env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        contract_id_1.clone(),
                        Symbol::new(&env, "partial_auth_sub_auth"),
                        (&user, &val).into_val(&env),
                    )),
                    sub_invocations: std::vec![AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            contract_id_2.clone(),
                            Symbol::new(&env, "do_auth"),
                            (&user, &val).into_val(&env),
                        )),
                        sub_invocations: std::vec![],
                    }],
                },
            )]
        );
        assert_eq!(res, val);
    }

    #[test]
    fn test_diff_auth_sub_auth_creates_expected_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let user = Address::generate(&env);
        let val = Symbol::new(&env, "test_auth");

        let contract_id_1 = env.register(AuthContract, (user.clone(),));
        let client_1 = AuthContractClient::new(&env, &contract_id_1);
        let contract_id_2 = env.register(AuthContract, (user.clone(),));

        let res = client_1.diff_auth_sub_auth(&user, &val, &contract_id_2);
        assert_eq!(
            env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        contract_id_1.clone(),
                        Symbol::new(&env, "diff_auth_sub_auth"),
                        (&1i128, &2i128).into_val(&env),
                    )),
                    sub_invocations: std::vec![AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            contract_id_2.clone(),
                            Symbol::new(&env, "do_auth"),
                            (&user, &val).into_val(&env),
                        )),
                        sub_invocations: std::vec![],
                    }],
                },
            )]
        );
        assert_eq!(res, val);
    }

    #[test]
    fn test_no_auth_sub_auth_creates_expected_auth() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let user = Address::generate(&env);
        let val = Symbol::new(&env, "test_auth");

        let contract_id_1 = env.register(AuthContract, (user.clone(),));
        let client_1 = AuthContractClient::new(&env, &contract_id_1);
        let contract_id_2 = env.register(AuthContract, (user.clone(),));

        let res = client_1.no_auth_sub_auth(&user, &val, &contract_id_2);
        assert_eq!(
            env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        contract_id_2.clone(),
                        Symbol::new(&env, "do_auth"),
                        (&user, &val).into_val(&env),
                    )),
                    sub_invocations: std::vec![],
                },
            )]
        );
        assert_eq!(res, val);
    }

    #[test]
    fn test_no_auth_sub_nested_auth_creates_expected_auth() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let user = Address::generate(&env);
        let val = Symbol::new(&env, "test_auth");

        let contract_id_1 = env.register(AuthContract, (user.clone(),));
        let client_1 = AuthContractClient::new(&env, &contract_id_1);
        let contract_id_2 = env.register(AuthContract, (user.clone(),));
        let contract_id_3 = env.register(AuthContract, (user.clone(),));

        let res = client_1.no_auth_sub_nested_auth(&user, &val, &contract_id_2, &contract_id_3);
        assert_eq!(
            env.auths(),
            std::vec![(
                user.clone(),
                AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        contract_id_2.clone(),
                        Symbol::new(&env, "auth_sub_auth"),
                        (&user, &val, &contract_id_3).into_val(&env),
                    )),
                    sub_invocations: std::vec![AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            contract_id_3.clone(),
                            Symbol::new(&env, "do_auth"),
                            (&user, &val).into_val(&env),
                        )),
                        sub_invocations: std::vec![],
                    }],
                },
            )]
        );
        assert_eq!(res, val);
    }
}
