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

type mockEntryReadTx struct{}

func (m mockEntryReadTx) GetLatestLedgerSequence() (uint32, error) {
	return 2, nil
}

func (m mockEntryReadTx) GetLedgerEntry(key xdr.LedgerKey) (bool, xdr.LedgerEntry, error) {
	switch key.Type {
	case xdr.LedgerEntryTypeContractData:
		if key.ContractData.Key.Type == xdr.ScValTypeScvLedgerKeyContractExecutable {
			entry := xdr.LedgerEntry{
				LastModifiedLedgerSeq: 1,
				Data: xdr.LedgerEntryData{
					Type: xdr.LedgerEntryTypeContractData,
					ContractData: &xdr.ContractDataEntry{
						ContractId: mockContractID,
						Key: xdr.ScVal{
							Type: xdr.ScValTypeScvLedgerKeyContractExecutable,
						},
						Val: xdr.ScVal{
							Type: xdr.ScValTypeScvContractExecutable,
							Exec: &xdr.ScContractExecutable{
								Type:   xdr.ScContractExecutableTypeSccontractExecutableWasmRef,
								WasmId: &mockContractHash,
							},
						},
					},
				},
			}
			return true, entry, nil
		}

	case xdr.LedgerEntryTypeContractCode:
		entry := xdr.LedgerEntry{
			LastModifiedLedgerSeq: 2,
			Data: xdr.LedgerEntryData{
				Type: xdr.LedgerEntryTypeContractCode,
				ContractCode: &xdr.ContractCodeEntry{
					Hash: mockContractHash,
					Code: helloWorldContract,
				},
			},
		}
		return true, entry, nil
	case xdr.LedgerEntryTypeConfigSetting:
		switch key.ConfigSetting.ConfigSettingId {
		case xdr.ConfigSettingIdConfigSettingContractComputeV0:
			entry := xdr.LedgerEntry{
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
			}
			return true, entry, nil
		case xdr.ConfigSettingIdConfigSettingContractLedgerCostV0:
			entry := xdr.LedgerEntry{
				LastModifiedLedgerSeq: 2,
				Data: xdr.LedgerEntryData{
					Type: xdr.LedgerEntryTypeConfigSetting,
					ConfigSetting: &xdr.ConfigSettingEntry{
						ConfigSettingId: xdr.ConfigSettingIdConfigSettingContractLedgerCostV0,
						ContractLedgerCost: &xdr.ConfigSettingContractLedgerCostV0{
							LedgerMaxReadLedgerEntries:  100,
							LedgerMaxReadBytes:          100,
							LedgerMaxWriteLedgerEntries: 100,
							LedgerMaxWriteBytes:         100,
							TxMaxReadLedgerEntries:      100,
							TxMaxReadBytes:              100,
							TxMaxWriteLedgerEntries:     100,
							TxMaxWriteBytes:             100,
							FeeReadLedgerEntry:          100,
							FeeWriteLedgerEntry:         100,
							FeeRead1Kb:                  100,
							FeeWrite1Kb:                 100,
							BucketListSizeBytes:         100,
							BucketListFeeRateLow:        100,
							BucketListFeeRateHigh:       100,
							BucketListGrowthFactor:      100,
						},
					},
				},
			}
			return true, entry, nil
		case xdr.ConfigSettingIdConfigSettingContractHistoricalDataV0:
			entry := xdr.LedgerEntry{
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
			}
			return true, entry, nil
		case xdr.ConfigSettingIdConfigSettingContractMetaDataV0:
			entry := xdr.LedgerEntry{
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
			}
			return true, entry, nil
		case xdr.ConfigSettingIdConfigSettingContractBandwidthV0:
			entry := xdr.LedgerEntry{
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
			}
			return true, entry, nil
		}

	}
	return false, xdr.LedgerEntry{}, nil
}

func (m mockEntryReadTx) Done() error {
	return nil
}

func BenchmarkGetPreflight(b *testing.B) {
	methodSymbol := xdr.ScSymbol("hello")
	argSymbol := xdr.ScSymbol("world")
	contractIDBytes := xdr.ScBytes(mockContractID[:])
	parameters := xdr.ScVec{
		xdr.ScVal{
			Type:  xdr.ScValTypeScvBytes,
			Bytes: &contractIDBytes,
		},
		xdr.ScVal{
			Type: xdr.ScValTypeScvSymbol,
			Sym:  &methodSymbol,
		},
		xdr.ScVal{
			Type: xdr.ScValTypeScvSymbol,
			Sym:  &argSymbol,
		},
	}
	params := PreflightParameters{
		Logger:        log.New(),
		SourceAccount: xdr.MustAddress("GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"),
		InvokeHostFunction: xdr.InvokeHostFunctionOp{
			Functions: []xdr.HostFunction{
				{
					Args: xdr.HostFunctionArgs{
						Type:           xdr.HostFunctionTypeHostFunctionTypeInvokeContract,
						InvokeContract: &parameters,
					},
				},
			},
		},
		NetworkPassphrase: "foo",
		LedgerEntryReadTx: mockEntryReadTx{},
	}
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		b.StartTimer()
		_, err := GetPreflight(context.Background(), params)
		b.StopTimer()
		require.NoError(b, err)
	}
}
