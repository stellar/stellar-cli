use crate::xdr::{
    ContractEventBody, DiagnosticEvent, ScSymbol, ScVal, SorobanResources, TransactionMeta,
};
use soroban_rpc::GetTransactionResponse;
use std::fmt::{Debug, Display};

struct SimulatedCost<'a>(&'a SorobanResources);

impl Debug for SimulatedCost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Should we output the footprint here?
        writeln!(f, "=============== Simulated Cost ===============")?;
        writeln!(f, "CPU used: {}", self.0.instructions)?;
        writeln!(f, "Bytes read: {}", self.0.disk_read_bytes)?;
        writeln!(f, "Bytes written: {}", self.0.write_bytes)?;
        writeln!(f, "==============================================")?;
        Ok(())
    }
}

impl Display for SimulatedCost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Default, Clone, PartialEq, Eq)]
pub struct ActualCost {
    pub cpu_insns: Option<u64>,
    pub mem_bytes: Option<u64>,
    pub bytes_read: Option<u64>,
    pub bytes_written: Option<u64>,
}

impl ActualCost {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cpu_insns.is_none()
            && self.mem_bytes.is_none()
            && self.bytes_read.is_none()
            && self.bytes_written.is_none()
    }

    #[must_use]
    pub fn from_diagnostic_events(events: &[DiagnosticEvent]) -> Self {
        let mut actual = Self::default();
        for event in events {
            let ContractEventBody::V0(body) = &event.event.body;
            if body.topics.len() >= 2 {
                if let (Some(ScVal::Symbol(ScSymbol(cat))), Some(ScVal::Symbol(ScSymbol(metric)))) =
                    (body.topics.first(), body.topics.get(1))
                {
                    if cat.as_slice() == b"core_metrics" {
                        let value = match &body.data {
                            ScVal::U64(v) => Some(*v),
                            _ => None,
                        };
                        match metric.as_slice() {
                            b"cpu_insn" => actual.cpu_insns = value,
                            b"mem_byte" => actual.mem_bytes = value,
                            b"ledger_read_byte" => actual.bytes_read = value,
                            b"ledger_write_byte" => actual.bytes_written = value,
                            _ => {}
                        }
                    }
                }
            }
        }
        actual
    }

    #[must_use]
    pub fn from_tx_meta(meta: &TransactionMeta) -> Self {
        let events = crate::log::extract_events(meta);
        Self::from_diagnostic_events(&events)
    }

    #[must_use]
    pub fn from_tx_response(resp: &GetTransactionResponse) -> Self {
        let mut actual = Self::from_diagnostic_events(&resp.events.diagnostic_events);
        if actual.is_empty() {
            if let Some(meta) = &resp.result_meta {
                actual = Self::from_tx_meta(meta);
            }
        }
        actual
    }
}

impl Debug for ActualCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "================= Actual Cost =================")?;
        if let Some(cpu) = self.cpu_insns {
            writeln!(f, "CPU used: {cpu}")?;
        }
        if let Some(mem) = self.mem_bytes {
            writeln!(f, "Memory used: {mem}")?;
        }
        if let Some(read) = self.bytes_read {
            writeln!(f, "Bytes read: {read}")?;
        }
        if let Some(written) = self.bytes_written {
            writeln!(f, "Bytes written: {written}")?;
        }
        writeln!(f, "===============================================")?;
        Ok(())
    }
}

impl Display for ActualCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

/// Logs the simulated cost calculated during transaction simulation.
pub fn cost(resources: &SorobanResources) {
    let cost = SimulatedCost(resources);
    tracing::debug!(?cost);
}

/// Logs the simulated cost calculated during transaction simulation.
pub fn simulated_cost(resources: &SorobanResources) {
    cost(resources);
}

/// Logs the actual cost incurred as reported by the transaction meta or diagnostic events.
pub fn actual_cost(cost: &ActualCost) {
    if !cost.is_empty() {
        tracing::debug!(?cost);
    }
}

/// Extracts and logs actual cost from diagnostic events.
pub fn actual_from_events(events: &[DiagnosticEvent]) {
    let actual = ActualCost::from_diagnostic_events(events);
    actual_cost(&actual);
}

/// Extracts and logs actual cost from transaction meta.
pub fn actual_from_meta(meta: &TransactionMeta) {
    let actual = ActualCost::from_tx_meta(meta);
    actual_cost(&actual);
}

/// Extracts and logs actual cost from a transaction response.
pub fn actual_from_tx_response(resp: &GetTransactionResponse) {
    let actual = ActualCost::from_tx_response(resp);
    actual_cost(&actual);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        ContractEvent, ContractEventType, ContractEventV0, ExtensionPoint, LedgerFootprint, VecM,
    };

    fn make_metric_event(name: &str, val: u64) -> DiagnosticEvent {
        DiagnosticEvent {
            in_successful_contract_call: true,
            event: ContractEvent {
                ext: ExtensionPoint::V0,
                contract_id: None,
                type_: ContractEventType::Diagnostic,
                body: ContractEventBody::V0(ContractEventV0 {
                    topics: vec![
                        ScVal::Symbol(ScSymbol("core_metrics".try_into().unwrap())),
                        ScVal::Symbol(ScSymbol(name.try_into().unwrap())),
                    ]
                    .try_into()
                    .unwrap(),
                    data: ScVal::U64(val),
                }),
            },
        }
    }

    #[test]
    fn test_simulated_cost_display() {
        let resources = SorobanResources {
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: VecM::default(),
            },
            instructions: 6104747,
            disk_read_bytes: 6976,
            write_bytes: 0,
        };
        let cost = SimulatedCost(&resources);
        let s = format!("{cost}");
        assert!(s.contains("=============== Simulated Cost ==============="));
        assert!(s.contains("CPU used: 6104747"));
        assert!(s.contains("Bytes read: 6976"));
        assert!(s.contains("Bytes written: 0"));
    }

    #[test]
    fn test_actual_cost_from_events() {
        let events = vec![
            make_metric_event("cpu_insn", 8808582),
            make_metric_event("mem_byte", 3010311),
            make_metric_event("ledger_read_byte", 53060),
            make_metric_event("ledger_write_byte", 1276),
        ];

        let actual = ActualCost::from_diagnostic_events(&events);
        assert_eq!(actual.cpu_insns, Some(8808582));
        assert_eq!(actual.mem_bytes, Some(3010311));
        assert_eq!(actual.bytes_read, Some(53060));
        assert_eq!(actual.bytes_written, Some(1276));
        assert!(!actual.is_empty());

        let s = format!("{actual}");
        assert!(s.contains("================= Actual Cost ================="));
        assert!(s.contains("CPU used: 8808582"));
        assert!(s.contains("Memory used: 3010311"));
        assert!(s.contains("Bytes read: 53060"));
        assert!(s.contains("Bytes written: 1276"));
    }

    #[test]
    fn test_empty_actual_cost() {
        let actual = ActualCost::default();
        assert!(actual.is_empty());
        let events = vec![DiagnosticEvent {
            in_successful_contract_call: true,
            event: ContractEvent {
                ext: ExtensionPoint::V0,
                contract_id: None,
                type_: ContractEventType::Diagnostic,
                body: ContractEventBody::V0(ContractEventV0 {
                    topics: VecM::default(),
                    data: ScVal::Void,
                }),
            },
        }];
        let actual_from_empty = ActualCost::from_diagnostic_events(&events);
        assert!(actual_from_empty.is_empty());
    }
}
