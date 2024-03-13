import test from "ava"
import { rpcUrl, root, signer } from "./util.js"
import { Client, networks } from "test-hello-world"

const contract = new Client({
  rpcUrl,
  allowHttp: true,
  publicKey: root.keypair.publicKey(),
  ...signer,
  ...networks.standalone,
})

test("has correctly-typed result", async (t) => {
  const initial = await contract.hello({ world: "tests" })
  t.is(initial.result[0], "Hello")
  t.is(initial.result[1], "tests")

  const serialized = initial.toJSON()
  const deserialized = contract.fromJSON.hello(serialized)
  t.is(deserialized.result[0], "Hello") // throws TS error if `result` is of type `unknown`
  t.is(deserialized.result[1], "tests") // throws TS error if `result` is of type `unknown`
});
