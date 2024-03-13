import test from "ava";
import { root, signer, rpcUrl } from "./util.js";
import { Client, networks } from "test-hello-world";

const contract = new Client({
  ...networks.standalone,
  rpcUrl,
  allowHttp: true,
  publicKey: root.keypair.publicKey(),
  ...signer,
})

test("hello", async (t) => {
  t.deepEqual((await contract.hello({ world: "tests" })).result, ["Hello", "tests"]);
});

test("auth", async (t) => {
  t.deepEqual(
    (await contract.auth({
      addr: root.keypair.publicKey(),
      world: 'lol'
    })).result,
    root.keypair.publicKey()
  )
});

test("inc", async (t) => {
  const { result: startingBalance } = await contract.get_count()
  const inc = await contract.inc()
  t.is((await inc.signAndSend()).result, startingBalance + 1)
  t.is((await contract.get_count()).result, startingBalance + 1)
});
