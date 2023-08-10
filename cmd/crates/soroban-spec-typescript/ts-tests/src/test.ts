import test from 'ava'
import { Contract, Ok, Err } from '../../fixtures/test_custom_types/dist/esm/index.js'

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

test('u32_fail_on_even', async t => {
  t.deepEqual(await contract.u32FailOnEven({ u32_: 1 }), new Ok(1))
  t.deepEqual(await contract.u32FailOnEven({ u32_: 0 }), new Err({ message: "Please provide an odd number" }))
})

test('u32', async t => {
  t.is(await contract.u32({ u32_: 1 }), 1)
})

test('i32', async t => {
  t.is(await contract.i32({ i32_: 1 }), 1)
})

test('i64', async t => {
  t.is(await contract.i64({ i64_: 1n }), 1n)
})

test("strukt_hel", async (t) => {
  let test = { a: 0, b: true, c: "world" }
  t.deepEqual(await contract.struktHel({ strukt: test }), ["Hello", "world"])
})

test.failing("strukt", async (t) => {
  let test = { a: 0, b: true, c: "hello" }
  t.deepEqual(await contract.strukt({ strukt: test }), test)
})

test('simple first', async t => {
  const simple = { tag: 'First', values: undefined } as const
  t.deepEqual(await contract.simple({ simple }), simple)
})

test('simple second', async t => {
  const simple = { tag: 'Second', values: undefined } as const
  t.deepEqual(await contract.simple({ simple }), simple)
})

test('simple third', async t => {
  const simple = { tag: 'Third', values: undefined } as const
  t.deepEqual(await contract.simple({ simple }), simple)
})

test('complex with struct', async t => {
  const arg = { tag: 'Struct', values: [{ a: 0, b: true, c: 'hello' }] } as const
  const ret = { tag: 'Struct', values: { a: 0, b: true, c: 'hello' } }
  t.deepEqual(await contract.complex({ complex: arg }), ret)
})

test('complex with tuple', async t => {
  const arg = { tag: 'Tuple', values: [[{ a: 0, b: true, c: 'hello' }, { tag: 'First', values: undefined }]] } as const
  const ret = { tag: 'Tuple', values: [{ a: 0, b: true, c: 'hello' }, ['First']] }
  t.deepEqual(await contract.complex({ complex: arg }), ret)
})

test('complex with enum', async t => {
  const arg = { tag: 'Enum', values: [{ tag: 'First', values: undefined }] } as const
  const ret = { tag: 'Enum', values: ['First'] }
  t.deepEqual(await contract.complex({ complex: arg }), ret)
})

test('complex with asset', async t => {
  const arg = { tag: 'Asset', values: [publicKey, 1n] } as const
  const ret = { tag: 'Asset', values: publicKey }
  t.deepEqual(await contract.complex({ complex: arg }), ret)
})

test('complex with void', async t => {
  const complex = { tag: 'Void', values: undefined } as const
  t.deepEqual(await contract.complex({ complex }), complex)
})

test.todo('addresse')

test.todo('bytes')

test.todo('bytes_n')

test.todo('card')

test.todo('boolean')

test.todo('not')

test.todo('i128')

test.todo('u128')

test.todo('multi_args')

test.todo('map')

test.todo('vec')

test.todo('tuple')

test.todo('option')

test.todo('u256')

test.todo('i256')

test.todo('string')

test.todo('tuple_strukt')