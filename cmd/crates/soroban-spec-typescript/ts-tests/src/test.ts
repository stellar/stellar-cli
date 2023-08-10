import test from 'ava'
import { Contract, Ok, Err, Error_ } from '../../fixtures/test_custom_types/dist/esm/index.js'

// hash of installed `test_custom_types` contract
// const CONTRACT_HASH = '693a01aa9c1388acbce3d84f045ff6e66579ad06a8dd3adac9fbdd793e72705f'
const contractId = 'CB5T6MLZNWJBUBKEQAUVIG5JJWKYSYVVE2OVN25GMX3VX7CZ7OBAPAU4'
const rpcUrl = 'https://rpc-futurenet.stellar.org'
const networkPassphrase = 'Test SDF Future Network ; October 2022'
const seedPhrase = 'jealous isolate reflect gun lazy genre strategy real phone like flame cheese'
const publicKey = 'GCBVOLOM32I7OD5TWZQCIXCXML3TK56MDY7ZMTAILIBQHHKPCVU42XYW'

const contract = new Contract({
  contractId,
  rpcUrl,
  networkPassphrase,
  wallet: {
    isConnected: () => Promise.resolve(true),
    isAllowed: () => Promise.resolve(true),
    getUserInfo: () => Promise.resolve({ publicKey }),
    signTransaction: async (tx: string, opts?: {
      network?: string,
      networkPassphrase?: string,
      accountToSign?: string,
    }) => {
      console.log(`how to use account "${seedPhrase}" to sign tx?`, tx)
      return tx
    }
  },
})

test('hello', async t => {
  t.is(await contract.hello({ hello: 'tests' }), 'tests')
})

test('woid', async t => {
  t.is(await contract.woid(), undefined)
})

test("stukt", async (t) => {
  let test = { a: 0, b: true, c: "hello" };
  t.notDeepEqual(await contract.strukt({ strukt: test }), test);
});

test('u32_fail_on_even', async t => {
  t.deepEqual(await contract.u32FailOnEven({ u32_: 1 }), new Ok(1))
  t.deepEqual(await contract.u32FailOnEven({ u32_: 0 }), new Err({ message: "Please provide an odd number" }))
})