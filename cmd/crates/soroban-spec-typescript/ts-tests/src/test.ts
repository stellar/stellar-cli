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

test('addresse', async t => {
  t.is(await contract.addresse({ addresse: publicKey }), publicKey)
})

test('bytes', async t => {
  const bytes = Buffer.from('hello')
  t.deepEqual(await contract.bytes({ bytes }), bytes)
})

test.failing('bytes_n', async t => {
  const bytes_n = Buffer.from('1') // what's the correct way to construct bytes_n?
  t.deepEqual(await contract.bytesN({ bytes_n }), bytes_n)
})

test.failing('card', async t => {
  const card = 11
  t.is(await contract.card({ card }), card)
})

test('boolean', async t => {
  t.is(await contract.boolean({ boolean: true }), true)
})

test('not', async t => {
  t.is(await contract.not({ boolean: true }), false)
})

test('i128', async t => {
  t.is(await contract.i128({ i128: -1n }), -1n)
})

test('u128', async t => {
  t.is(await contract.u128({ u128: 1n }), 1n)
})

test('multi_args', async t => {
  t.is(await contract.multiArgs({ a: 1, b: true }), 1)
  t.is(await contract.multiArgs({ a: 1, b: false }), 0)
})

test('map', async t => {
  const map = new Map()
  map.set(1, true)
  map.set(2, false)
  // map.set(3, 'hahaha') // should throw an error
  t.deepEqual(await contract.map({ map }), Object.fromEntries(map.entries()))
})

test('vec', async t => {
  const vec = [1, 2, 3]
  t.deepEqual(await contract.vec({ vec }), vec)
})

test('tuple', async t => {
  const tuple = ['hello', 1] as const
  t.deepEqual(await contract.tuple({ tuple }), tuple)
})

test.failing('option', async t => {
  // this makes sense
  t.deepEqual(await contract.option({ option: 1 }), 1)

  // this passes but shouldn't
  t.deepEqual(await contract.option({ option: undefined }), 0)

  // this is the behavior we probably want, but fails
  // t.deepEqual(await contract.option(), undefined) // typing and implementation require the object
  // t.deepEqual(await contract.option({}), undefined) // typing requires argument; implementation would be fine with this
  t.deepEqual(await contract.option({ option: undefined }), undefined)
})

test.failing('u256', async t => {
  t.is(await contract.u256({ u256: 1n }), 1n)
})

test.failing('i256', async t => {
  t.is(await contract.i256({ i256: -1n }), -1n)
})

test('string', async t => {
  t.is(await contract.string({ string: 'hello' }), 'hello')
})

test('tuple_strukt', async t => {
  const arg = [{ a: 0, b: true, c: 'hello' }, { tag: 'First', values: undefined }] as const
  const res = [{ a: 0, b: true, c: 'hello' }, ['First']]
  t.deepEqual(await contract.tupleStrukt({ tuple_strukt: arg }), res)
})