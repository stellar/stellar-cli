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

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

var mockContractID = xdr.Hash{0xa, 0xb, 0xc}
var mockContractHash = xdr.Hash{0xd, 0xe, 0xf}

var contractCostParams = func() *xdr.ContractCostParams {
	var result xdr.ContractCostParams

	for i := 0; i < 22; i++ {
		result = append(result, xdr.ContractCostParamEntry{
			Ext:        xdr.ExtensionPoint{},
			ConstTerm:  0,
			LinearTerm: 0,
		})
	}

	return &result
}()

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
				Durability:          xdr.ContractDataDurabilityPersistent,
				ExpirationLedgerSeq: 100000,
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
					LedgerMaxInstructions:           100000000,
					TxMaxInstructions:               100000000,
					FeeRatePerInstructionsIncrement: 1,
					TxMemoryLimit:                   100000000,
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
					WriteFee1KbBucketListLow:       1,
					WriteFee1KbBucketListHigh:      1,
					BucketListWriteFeeGrowthFactor: 1,
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
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingStateExpiration,
				StateExpirationSettings: &xdr.StateExpirationSettings{
					MaxEntryExpiration:             100,
					MinTempEntryExpiration:         100,
					MinPersistentEntryExpiration:   100,
					AutoBumpLedgers:                100,
					PersistentRentRateDenominator:  100,
					TempRentRateDenominator:        100,
					MaxEntriesToExpire:             100,
					BucketListSizeWindowSampleSize: 100,
					EvictionScanSize:               100,
				},
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractCostParamsCpuInstructions,
				// Obtained with TestGetLedgerEntryConfigSettings
				ContractCostParamsCpuInsns: contractCostParams,
			},
		},
	},
	{
		LastModifiedLedgerSeq: 2,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeConfigSetting,
			ConfigSetting: &xdr.ConfigSettingEntry{
				ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractCostParamsMemoryBytes,
				// Obtained with TestGetLedgerEntryConfigSettings
				ContractCostParamsMemBytes: contractCostParams,
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

func getPreflightParameters(t testing.TB, inMemory bool) PreflightParameters {
	var ledgerEntryReadTx db.LedgerEntryReadTx
	if inMemory {
		var err error
		ledgerEntryReadTx, err = newInMemoryLedgerEntryReadTx(mockLedgerEntries)
		require.NoError(t, err)
	} else {
		d := t.TempDir()
		dbInstance, err := db.OpenSQLiteDB(path.Join(d, "soroban_rpc.sqlite"))
		require.NoError(t, err)
		readWriter := db.NewReadWriter(dbInstance, 100, 10000)
		tx, err := readWriter.NewTx(context.Background())
		require.NoError(t, err)
		for _, e := range mockLedgerEntries {
			err := tx.LedgerEntryWriter().UpsertLedgerEntry(e)
			require.NoError(t, err)
		}
		err = tx.Commit(2)
		require.NoError(t, err)
		ledgerEntryReadTx, err = db.NewLedgerEntryReader(dbInstance).NewCachedTx(context.Background())
		require.NoError(t, err)
	}
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
		BucketListSize:    200,
	}
	return params
}

func TestGetPreflight(t *testing.T) {
	params := getPreflightParameters(t, false)
	_, err := GetPreflight(context.Background(), params)
	require.NoError(t, err)

	params = getPreflightParameters(t, true)
	_, err = GetPreflight(context.Background(), params)
	require.NoError(t, err)
}

func benchmark(b *testing.B, inMemory bool) {
	params := getPreflightParameters(b, inMemory)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		b.StartTimer()
		_, err := GetPreflight(context.Background(), params)
		b.StopTimer()
		require.NoError(b, err)
	}
}

func BenchmarkGetPreflight(b *testing.B) {
	b.Run("In-memory storage", func(b *testing.B) { benchmark(b, true) })
	b.Run("DB storage", func(b *testing.B) { benchmark(b, false) })
}
