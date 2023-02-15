package memorystore

import "github.com/stellar/go/xdr"

type Transaction struct {
	id       xdr.Hash
	envelope xdr.TransactionEnvelope
	result   xdr.TransactionResult
	meta     xdr.TransactionMeta
}

func readTransactions(ledgerCloseMeta xdr.LedgerCloseMeta) []Transaction {
	envs := ledgerCloseMeta.TransactionEnvelopes()
	result := make([]Transaction, len(envs))
	for i := range envs {
		resultPair := ledgerCloseMeta.TransactionResultPair(i)
		result[i].id = resultPair.TransactionHash
		result[i].envelope = envs[i]
		result[i].result = resultPair.Result
		result[i].meta = ledgerCloseMeta.TxApplyProcessing(i)
	}
	return result
}
