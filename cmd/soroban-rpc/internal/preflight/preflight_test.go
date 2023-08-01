package preflight

import (
	"context"
	"os"
	"path"
	"runtime"
	"testing"

	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/require"
)

var mockContractID = xdr.Hash{0xa, 0xb, 0xc}
var mockContractHash = xdr.Hash{0xd, 0xe, 0xf}

var mockLedgerEntries = []xdr.LedgerEntry{
	{
		LastModifiedLedgerSeq: 1,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeContractData,
			ContractData: &xdr.ContractDataEntry{
				Contract: xdr.ScAddress{
					Type:       xdr.ScAddressTypeScAddressTypeContract,
					ContractId: &mockContractID,
				},
				Key: xdr.ScVal{
					Type: xdr.ScValTypeScvLedgerKeyContractInstance,
				},
				Durability: xdr.ContractDataDurabilityPersistent,
				Body: xdr.ContractDataEntryBody{
					BodyType: xdr.ContractEntryBodyTypeDataEntry,
					Data: &xdr.ContractDataEntryData{
						Flags: 0,
						Val: xdr.ScVal{
							Type: xdr.ScValTypeScvContractInstance,
							Instance: &xdr.ScContractInstance{
								Executable: xdr.ContractExecutable{
									Type:     xdr.ContractExecutableTypeContractExecutableWasm,
									WasmHash: &mockContractHash,
								},
								Storage: nil,
							},
						},
					},
				},
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeContractCode,
			ContractCode: &xdr.ContractCodeEntry{
				Hash: mockContractHash,
				Body: xdr.ContractCodeEntryBody{
					BodyType: xdr.ContractEntryBodyTypeDataEntry,
					Code:     &helloWorldContract,
				},
				ExpirationLedgerSeq: 20000,
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractComputeV0,
				ContractCompute: &xdr.ConfigSettingContractComputeV0{
					LedgerMaxInstructions:           100,
					TxMaxInstructions:               100,
					FeeRatePerInstructionsIncrement: 100,
					TxMemoryLimit:                   100,
				},
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractLedgerCostV0,
				ContractLedgerCost: &xdr.ConfigSettingContractLedgerCostV0{
					LedgerMaxReadLedgerEntries:     100,
					LedgerMaxReadBytes:             100,
					LedgerMaxWriteLedgerEntries:    100,
					LedgerMaxWriteBytes:            100,
					TxMaxReadLedgerEntries:         100,
					TxMaxReadBytes:                 100,
					TxMaxWriteLedgerEntries:        100,
					TxMaxWriteBytes:                100,
					FeeReadLedgerEntry:             100,
					FeeWriteLedgerEntry:            100,
					FeeRead1Kb:                     100,
					BucketListTargetSizeBytes:      100,
					WriteFee1KbBucketListLow:       100,
					WriteFee1KbBucketListHigh:      100,
					BucketListWriteFeeGrowthFactor: 100,
				},
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractHistoricalDataV0,
				ContractHistoricalData: &xdr.ConfigSettingContractHistoricalDataV0{
					FeeHistorical1Kb: 100,
				},
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractMetaDataV0,
				ContractMetaData: &xdr.ConfigSettingContractMetaDataV0{
					TxMaxExtendedMetaDataSizeBytes: 100,
					FeeExtendedMetaData1Kb:         100,
				},
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractBandwidthV0,
				ContractBandwidth: &xdr.ConfigSettingContractBandwidthV0{
					LedgerMaxPropagateSizeBytes: 100,
					TxMaxSizeBytes:              100,
					FeePropagateData1Kb:         100,
				},
			},
		},
	},
}

var helloWorldContract = func() []byte {
	_, filename, _, _ := runtime.Caller(0)
	testDirName := path.Dir(filename)
	contractFile := path.Join(testDirName, "../../../../target/wasm32-unknown-unknown/test-wasms/test_hello_world.wasm")
	ret, err := os.ReadFile(contractFile)
	if err != nil {
		log.Fatalf("unable to read test_hello_world.wasm (%v) please run `make build-test-wasms` at the project root directory", err)
	}
	return ret
}()

type inMemoryLedgerEntryReadTx map[string]xdr.LedgerEntry

func newInMemoryLedgerEntryReadTx(entries []xdr.LedgerEntry) (inMemoryLedgerEntryReadTx, error) {
	result := make(map[string]xdr.LedgerEntry, len(entries))
	for _, entry := range entries {
		key, err := entry.LedgerKey()
		if err != nil {
			return inMemoryLedgerEntryReadTx{}, err
		}
		serialized, err := key.MarshalBinaryBase64()
		if err != nil {
			return inMemoryLedgerEntryReadTx{}, err
		}
		result[serialized] = entry
	}
	return result, nil
}

func (m inMemoryLedgerEntryReadTx) GetLatestLedgerSequence() (uint32, error) {
	return 2, nil
}

func (m inMemoryLedgerEntryReadTx) GetLedgerEntry(key xdr.LedgerKey, includeExpired bool) (bool, xdr.LedgerEntry, error) {
	serializedKey, err := key.MarshalBinaryBase64()
	if err != nil {
		return false, xdr.LedgerEntry{}, err
	}
	entry, ok := m[serializedKey]
	if !ok {
		return false, xdr.LedgerEntry{}, nil
	}
	return true, entry, nil
}

func (m inMemoryLedgerEntryReadTx) Done() error {
	return nil
}

func BenchmarkGetPreflight(b *testing.B) {
	ledgerEntryReadTx, err := newInMemoryLedgerEntryReadTx(mockLedgerEntries)
	require.NoError(b, err)
	argSymbol := xdr.ScSymbol("world")
	params := PreflightParameters{
		Logger:        log.New(),
		SourceAccount: xdr.MustAddress("GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"),
		OpBody: xdr.OperationBody{Type: xdr.OperationTypeInvokeHostFunction,
			InvokeHostFunctionOp: &xdr.InvokeHostFunctionOp{
				HostFunction: xdr.HostFunction{
					Type: xdr.HostFunctionTypeHostFunctionTypeInvokeContract,
					InvokeContract: &xdr.InvokeContractArgs{
						ContractAddress: xdr.ScAddress{
							Type:       xdr.ScAddressTypeScAddressTypeContract,
							ContractId: &mockContractID,
						},
						FunctionName: "hello",
						Args: []xdr.ScVal{
							{
								Type: xdr.ScValTypeScvSymbol,
								Sym:  &argSymbol,
							},
						},
					},
				},
			}},
		NetworkPassphrase: "foo",
		LedgerEntryReadTx: ledgerEntryReadTx,
	}
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		b.StartTimer()
		_, err := GetPreflight(context.Background(), params)
		b.StopTimer()
		require.NoError(b, err)
	}
}
