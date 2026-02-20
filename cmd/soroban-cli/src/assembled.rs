use sha2::{Digest, Sha256};
use stellar_xdr::curr::{
    self as xdr, Hash, LedgerFootprint, Limits, OperationBody, ReadXdr, SorobanAuthorizationEntry,
    SorobanAuthorizedFunction, SorobanResources, SorobanTransactionData, Transaction,
    TransactionEnvelope, TransactionExt, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope, VecM, WriteXdr,
};

use soroban_rpc::{Error, LogEvents, LogResources, ResourceConfig, SimulateTransactionResponse};

pub async fn simulate_and_assemble_transaction(
    client: &soroban_rpc::Client,
    tx: &Transaction,
    resource_config: Option<ResourceConfig>,
    resource_fee: Option<i64>,
) -> Result<Assembled, Error> {
    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures: VecM::default(),
    });

    tracing::trace!(
        "Simulation transaction envelope: {}",
        envelope.to_xdr_base64(Limits::none())?
    );

    let sim_res = client
        .next_simulate_transaction_envelope(&envelope, None, resource_config)
        .await?;
    tracing::trace!("{sim_res:#?}");

    if let Some(e) = &sim_res.error {
        crate::log::event::all(&sim_res.events()?);
        Err(Error::TransactionSimulationFailed(e.clone()))
    } else {
        Ok(Assembled::new(tx, sim_res, resource_fee)?)
    }
}

pub struct Assembled {
    pub(crate) txn: Transaction,
    pub(crate) sim_res: SimulateTransactionResponse,
    pub(crate) fee_bump_fee: Option<i64>,
}

/// Represents an assembled transaction ready to be signed and submitted to the network.
impl Assembled {
    ///
    /// Creates a new `Assembled` transaction.
    ///
    /// # Arguments
    ///
    /// * `txn` - The original transaction.
    /// * `sim_res` - The simulation response.
    /// * `resource_fee` - Optional resource fee for the transaction. Will override the simulated resource fee if provided.
    ///
    /// # Errors
    ///
    /// Returns an error if simulation fails or if assembling the transaction fails.
    pub fn new(
        txn: &Transaction,
        sim_res: SimulateTransactionResponse,
        resource_fee: Option<i64>,
    ) -> Result<Self, Error> {
        assemble(txn, sim_res, resource_fee)
    }

    ///
    /// Calculates the hash of the assembled transaction.
    ///
    /// # Arguments
    ///
    /// * `network_passphrase` - The network passphrase.
    ///
    /// # Errors
    ///
    /// Returns an error if generating the hash fails.
    pub fn hash(&self, network_passphrase: &str) -> Result<[u8; 32], xdr::Error> {
        let signature_payload = TransactionSignaturePayload {
            network_id: Hash(Sha256::digest(network_passphrase).into()),
            tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(self.txn.clone()),
        };
        Ok(Sha256::digest(signature_payload.to_xdr(Limits::none())?).into())
    }

    /// Returns a reference to the original transaction.
    #[must_use]
    pub fn transaction(&self) -> &Transaction {
        &self.txn
    }

    /// Returns a reference to the simulation response.
    #[must_use]
    pub fn sim_response(&self) -> &SimulateTransactionResponse {
        &self.sim_res
    }

    #[must_use]
    pub fn fee_bump_fee(&self) -> Option<i64> {
        self.fee_bump_fee
    }

    #[must_use]
    pub fn bump_seq_num(mut self) -> Self {
        self.txn.seq_num.0 += 1;
        self
    }

    ///
    /// # Errors
    #[must_use]
    pub fn auth_entries(&self) -> VecM<SorobanAuthorizationEntry> {
        self.txn
            .operations
            .first()
            .and_then(|op| match op.body {
                OperationBody::InvokeHostFunction(ref body) => (matches!(
                    body.auth.first().map(|x| &x.root_invocation.function),
                    Some(&SorobanAuthorizedFunction::ContractFn(_))
                ))
                .then_some(body.auth.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    ///
    /// # Errors
    pub fn log(
        &self,
        log_events: Option<LogEvents>,
        log_resources: Option<LogResources>,
    ) -> Result<(), Error> {
        if let TransactionExt::V1(SorobanTransactionData {
            resources: resources @ SorobanResources { footprint, .. },
            ..
        }) = &self.txn.ext
        {
            if let Some(log) = log_resources {
                log(resources);
            }

            if let Some(log) = log_events {
                log(footprint, &[self.auth_entries()], &self.sim_res.events()?);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn is_view(&self) -> bool {
        let TransactionExt::V1(SorobanTransactionData {
            resources:
                SorobanResources {
                    footprint: LedgerFootprint { read_write, .. },
                    ..
                },
            ..
        }) = &self.txn.ext
        else {
            return false;
        };
        read_write.is_empty()
    }

    // TODO: Remove once `--instructions` is fully removed
    #[must_use]
    pub fn set_max_instructions(mut self, instructions: u32) -> Self {
        if let TransactionExt::V1(SorobanTransactionData {
            resources:
                SorobanResources {
                    instructions: ref mut i,
                    ..
                },
            ..
        }) = &mut self.txn.ext
        {
            tracing::trace!("setting max instructions to {instructions} from {i}");
            *i = instructions;
        }
        self
    }
}

// Apply the result of a simulateTransaction onto a transaction envelope, preparing it for
// submission to the network.
///
/// # Errors
fn assemble(
    raw: &Transaction,
    simulation: SimulateTransactionResponse,
    resource_fee: Option<i64>,
) -> Result<Assembled, Error> {
    let mut tx = raw.clone();

    // Right now simulate.results is one-result-per-function, and assumes there is only one
    // operation in the txn, so we need to enforce that here. I (Paul) think that is a bug
    // in soroban-rpc.simulateTransaction design, and we should fix it there.
    // TODO: We should to better handling so non-soroban txns can be a passthrough here.
    if tx.operations.len() != 1 {
        return Err(Error::UnexpectedOperationCount {
            count: tx.operations.len(),
        });
    }

    let mut transaction_data = simulation.transaction_data()?;
    let min_resource_fee = match resource_fee {
        Some(rf) => {
            tracing::trace!(
                "overriding resource fee to {rf} (simulation suggested {})",
                simulation.min_resource_fee
            );
            transaction_data.resource_fee = rf;
            // short circuit the submission error if the resource fee is negative
            // technically, a negative resource fee is valid XDR so it won't panic earlier
            // this should not occur as we validate resource fee before calling assemble
            u64::try_from(rf).map_err(|_| {
                Error::TransactionSubmissionFailed(String::from(
                    "TxMalformed - negative resource fee",
                ))
            })?
        }
        // transaction_data is already set from simulation response
        None => simulation.min_resource_fee,
    };

    let mut op = tx.operations[0].clone();
    if let OperationBody::InvokeHostFunction(ref mut body) = &mut op.body {
        if body.auth.is_empty() {
            if simulation.results.len() != 1 {
                return Err(Error::UnexpectedSimulateTransactionResultSize {
                    length: simulation.results.len(),
                });
            }

            let auths = simulation
                .results
                .iter()
                .map(|r| {
                    VecM::try_from(
                        r.auth
                            .iter()
                            .map(|v| SorobanAuthorizationEntry::from_xdr_base64(v, Limits::none()))
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !auths.is_empty() {
                body.auth = auths[0].clone();
            }
        }
    }

    // Update the transaction fee to be the sum of the inclusion fee and the
    // minimum resource fee from simulation.
    let total_fee: u64 = u64::from(raw.fee) + min_resource_fee;
    let mut fee_bump_fee: Option<i64> = None;
    if let Ok(tx_fee) = u32::try_from(total_fee) {
        tx.fee = tx_fee;
    } else {
        // Transaction needs a fee bump wrapper. Set the fee to 0 and assign the required fee
        // to the fee_bump_fee field, which will be used later when constructing the FeeBumpTransaction.
        // => fee_bump_fee = 2 * inclusion_fee + resource_fee
        tx.fee = 0;
        let fee_bump_fee_u64 = total_fee + u64::from(raw.fee);
        fee_bump_fee =
            Some(i64::try_from(fee_bump_fee_u64).map_err(|_| Error::LargeFee(fee_bump_fee_u64))?);
    }

    tx.operations = vec![op].try_into()?;
    tx.ext = TransactionExt::V1(transaction_data);
    Ok(Assembled {
        txn: tx,
        sim_res: simulation,
        fee_bump_fee,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use soroban_rpc::SimulateHostFunctionResultRaw;
    use stellar_strkey::ed25519::PublicKey as Ed25519PublicKey;
    use stellar_xdr::curr::{
        AccountId, ChangeTrustAsset, ChangeTrustOp, Hash, HostFunction, InvokeContractArgs,
        InvokeHostFunctionOp, LedgerFootprint, Memo, MuxedAccount, Operation, Preconditions,
        PublicKey, ScAddress, ScSymbol, ScVal, SequenceNumber, SorobanAuthorizedFunction,
        SorobanAuthorizedInvocation, SorobanResources, SorobanTransactionData, Uint256, WriteXdr,
    };

    const SOURCE: &str = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";

    fn transaction_data() -> SorobanTransactionData {
        SorobanTransactionData {
            resources: SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::default(),
                },
                instructions: 0,
                disk_read_bytes: 5,
                write_bytes: 0,
            },
            resource_fee: 0,
            ext: xdr::SorobanTransactionDataExt::V0,
        }
    }

    fn simulation_response() -> SimulateTransactionResponse {
        let source_bytes = Ed25519PublicKey::from_string(SOURCE).unwrap().0;
        let fn_auth = &SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
                address: ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                    source_bytes,
                )))),
                nonce: 0,
                signature_expiration_ledger: 0,
                signature: ScVal::Void,
            }),
            root_invocation: SorobanAuthorizedInvocation {
                function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                    contract_address: ScAddress::Contract(stellar_xdr::curr::ContractId(Hash(
                        [0; 32],
                    ))),
                    function_name: ScSymbol("fn".try_into().unwrap()),
                    args: VecM::default(),
                }),
                sub_invocations: VecM::default(),
            },
        };

        SimulateTransactionResponse {
            min_resource_fee: 115,
            latest_ledger: 3,
            results: vec![SimulateHostFunctionResultRaw {
                auth: vec![fn_auth.to_xdr_base64(Limits::none()).unwrap()],
                xdr: ScVal::U32(0).to_xdr_base64(Limits::none()).unwrap(),
            }],
            transaction_data: transaction_data().to_xdr_base64(Limits::none()).unwrap(),
            ..Default::default()
        }
    }

    fn single_contract_fn_transaction() -> Transaction {
        let source_bytes = Ed25519PublicKey::from_string(SOURCE).unwrap().0;
        Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(source_bytes)),
            fee: 100,
            seq_num: SequenceNumber(0),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: HostFunction::InvokeContract(InvokeContractArgs {
                        contract_address: ScAddress::Contract(stellar_xdr::curr::ContractId(Hash(
                            [0x0; 32],
                        ))),
                        function_name: ScSymbol::default(),
                        args: VecM::default(),
                    }),
                    auth: VecM::default(),
                }),
            }]
            .try_into()
            .unwrap(),
            ext: TransactionExt::V0,
        }
    }

    #[test]
    fn test_assemble_transaction_updates_tx_data_from_simulation_response() {
        let sim = simulation_response();
        let txn = single_contract_fn_transaction();
        let Ok(result) = assemble(&txn, sim, None) else {
            panic!("assemble failed");
        };

        // validate it auto updated the tx fees from sim response fees
        // since it was greater than tx.fee
        assert_eq!(215, result.txn.fee);

        // validate it updated sorobantransactiondata block in the tx ext
        assert_eq!(TransactionExt::V1(transaction_data()), result.txn.ext);
    }

    #[test]
    fn test_assemble_transaction_adds_the_auth_to_the_host_function() {
        let sim = simulation_response();
        let txn = single_contract_fn_transaction();
        let Ok(result) = assemble(&txn, sim, None) else {
            panic!("assemble failed");
        };

        assert_eq!(1, result.txn.operations.len());
        let OperationBody::InvokeHostFunction(ref op) = result.txn.operations[0].body else {
            panic!("unexpected operation type: {:#?}", result.txn.operations[0]);
        };

        assert_eq!(1, op.auth.len());
        let auth = &op.auth[0];

        let xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
            ref function_name,
            ..
        }) = auth.root_invocation.function
        else {
            panic!("unexpected function type");
        };
        assert_eq!("fn".to_string(), format!("{}", function_name.0));

        let xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
            address:
                xdr::ScAddress::Account(xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(address))),
            ..
        }) = &auth.credentials
        else {
            panic!("unexpected credentials type");
        };
        assert_eq!(
            SOURCE.to_string(),
            stellar_strkey::ed25519::PublicKey(address.0).to_string()
        );
    }

    #[test]
    fn test_assemble_transaction_errors_for_non_invokehostfn_ops() {
        let source_bytes = Ed25519PublicKey::from_string(SOURCE).unwrap().0;
        let txn = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(source_bytes)),
            fee: 100,
            seq_num: SequenceNumber(0),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::ChangeTrust(ChangeTrustOp {
                    line: ChangeTrustAsset::Native,
                    limit: 0,
                }),
            }]
            .try_into()
            .unwrap(),
            ext: TransactionExt::V0,
        };

        let result = assemble(
            &txn,
            SimulateTransactionResponse {
                min_resource_fee: 115,
                transaction_data: transaction_data().to_xdr_base64(Limits::none()).unwrap(),
                latest_ledger: 3,
                ..Default::default()
            },
            None,
        );

        match result {
            Ok(_) => {}
            Err(e) => panic!("expected assembled operation, got: {e:#?}"),
        }
    }

    #[test]
    fn test_assemble_transaction_errors_for_errors_for_mismatched_simulation() {
        let txn = single_contract_fn_transaction();

        let result = assemble(
            &txn,
            SimulateTransactionResponse {
                min_resource_fee: 115,
                transaction_data: transaction_data().to_xdr_base64(Limits::none()).unwrap(),
                latest_ledger: 3,
                ..Default::default()
            },
            None,
        );

        match result {
            Err(Error::UnexpectedSimulateTransactionResultSize { length }) => {
                assert_eq!(0, length);
            }
            Ok(_) => panic!("expected error, got success"),
            Err(e) => panic!("expected UnexpectedSimulateTransactionResultSize error, got: {e:#?}"),
        }
    }

    #[test]
    fn test_assemble_transaction_calcs_fee() {
        let mut sim = simulation_response();
        sim.min_resource_fee = 12345;
        let mut txn = single_contract_fn_transaction();
        txn.fee = 10000;
        let Ok(result) = assemble(&txn, sim, None) else {
            panic!("assemble failed");
        };

        assert_eq!(12345 + 10000, result.txn.fee);
        assert_eq!(None, result.fee_bump_fee);

        // validate it updated sorobantransactiondata block in the tx ext
        let expected_tx_data = transaction_data();
        assert_eq!(TransactionExt::V1(expected_tx_data), result.txn.ext);
    }

    #[test]
    fn test_assemble_transaction_fee_bump_fee_behavior() {
        // Test three separate cases:
        //
        //  1. Given a near-max (u32::MAX - 100) resource fee make sure the tx
        //     does not require a fee bump after adding the base inclusion fee (100).
        //  2. Given a large resource fee that WILL exceed u32::MAX with the
        //     base inclusion fee, ensure the fee is set to zero and the correct
        //     fee_bump_fee is set on the Assembled struct.
        //  3. Given a total fee over i64::MAX, ensure an error is returned.
        let mut txn = single_contract_fn_transaction();
        let mut response = simulation_response();

        let inclusion_fee: u32 = 500;
        let inclusion_fee_i64: i64 = i64::from(inclusion_fee);
        txn.fee = inclusion_fee;

        // 1: wiggle room math overflows but result fits
        response.min_resource_fee = (u32::MAX - inclusion_fee).into();

        match assemble(&txn, response.clone(), None) {
            Ok(assembled) => {
                assert_eq!(assembled.txn.fee, u32::MAX);
                assert_eq!(assembled.fee_bump_fee, None);
            }
            Err(e) => panic!("expected success, got error: {e:#?}"),
        }

        // 2: combo over u32::MAX, should set fee to 0 and fee_bump_fee to total
        response.min_resource_fee = (u32::MAX - inclusion_fee + 1).into();
        match assemble(&txn, response.clone(), None) {
            Ok(assembled) => {
                assert_eq!(assembled.txn.fee, 0);
                assert_eq!(
                    assembled.fee_bump_fee,
                    Some(i64::try_from(response.min_resource_fee).unwrap() + inclusion_fee_i64 * 2)
                );
            }
            Err(e) => panic!("expected success, got error: {e:#?}"),
        }

        // 3: total fee exceeds i64::MAX, should error
        response.min_resource_fee = u64::try_from(i64::MAX - (2 * inclusion_fee_i64) + 1).unwrap();
        match assemble(&txn, response, None) {
            Err(Error::LargeFee(fee)) => {
                let expected = i64::MAX as u64 + 1;
                assert_eq!(expected, fee, "expected {expected} != {fee} actual");
            }
            Ok(_) => panic!("expected error, got success"),
            Err(e) => panic!("expected success, got error: {e:#?}"),
        }
    }

    #[test]
    fn test_assemble_transaction_with_resource_fee() {
        let sim = simulation_response();
        let mut txn = single_contract_fn_transaction();
        txn.fee = 500;
        let resource_fee = 12345i64;
        let Ok(result) = assemble(&txn, sim, Some(resource_fee)) else {
            panic!("assemble failed");
        };

        // validate the assembled tx fee is the sum of the inclusion fee (txn.fee)
        // and the resource fee
        assert_eq!(12345 + 500, result.txn.fee);
        assert_eq!(None, result.fee_bump_fee);

        // validate it updated sorobantransactiondata block in the tx ext
        let mut expected_tx_data = transaction_data();
        expected_tx_data.resource_fee = resource_fee;
        assert_eq!(TransactionExt::V1(expected_tx_data), result.txn.ext);
    }

    // This should never occur, as resource fee is validated before being passed into
    // assemble. But test the behavior just in case.
    #[test]
    fn test_assemble_transaction_input_resource_fee_negative_errors() {
        let mut sim = simulation_response();
        sim.min_resource_fee = 12345;
        let mut txn = single_contract_fn_transaction();
        txn.fee = 500;
        let resource_fee = -1;
        let result = assemble(&txn, sim, Some(resource_fee));

        assert!(result.is_err());
    }

    #[test]
    fn test_assemble_transaction_with_resource_fee_fee_bump_behavior() {
        // Test three separate cases:
        //
        //  1. Given a near-max (u32::MAX - 100) resource fee make sure the tx
        //     does not require a fee bump after adding the base inclusion fee (100).
        //  2. Given a large resource fee that WILL exceed u32::MAX with the
        //     base inclusion fee, ensure the fee is set to zero and the correct
        //     fee_bump_fee is set on the Assembled struct.
        //  3. Given a total fee over i64::MAX, ensure an error is returned.
        let mut txn = single_contract_fn_transaction();
        let response = simulation_response();

        let inclusion_fee: u32 = 500;
        let inclusion_fee_i64: i64 = i64::from(inclusion_fee);
        txn.fee = inclusion_fee;

        // 1: wiggle room math overflows but result fits
        let resource_fee: i64 = (u32::MAX - inclusion_fee).into();
        match assemble(&txn, response.clone(), Some(resource_fee)) {
            Ok(assembled) => {
                assert_eq!(assembled.txn.fee, u32::MAX);
                assert_eq!(assembled.fee_bump_fee, None);
            }
            Err(e) => panic!("expected success, got error: {e:#?}"),
        }

        // 2: combo over u32::MAX, should set fee to 0 and fee_bump_fee to total
        let resource_fee: i64 = (u32::MAX - inclusion_fee + 1).into();
        match assemble(&txn, response.clone(), Some(resource_fee)) {
            Ok(assembled) => {
                assert_eq!(assembled.txn.fee, 0);
                assert_eq!(
                    assembled.fee_bump_fee,
                    Some(resource_fee + inclusion_fee_i64 * 2)
                );
            }
            Err(e) => panic!("expected success, got error: {e:#?}"),
        }

        // 3: total fee exceeds i64::MAX, should error
        let resource_fee: i64 = i64::MAX - (2 * inclusion_fee_i64) + 1;
        match assemble(&txn, response, Some(resource_fee)) {
            Err(Error::LargeFee(fee)) => {
                let expected = i64::MAX as u64 + 1;
                assert_eq!(expected, fee, "expected {expected} != {fee} actual");
            }
            Ok(_) => panic!("expected error, got success"),
            Err(e) => panic!("expected success, got error: {e:#?}"),
        }
    }
}
