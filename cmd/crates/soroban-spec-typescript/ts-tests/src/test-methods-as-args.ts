import test from "ava";
import { rpcUrl, root, signer } from "./util.js";
import { Client, networks } from "test-hello-world";

const contract = new Client({
  ...networks.standalone,
  rpcUrl,
  allowHttp: true,
  publicKey: root.keypair.publicKey(),
  ...signer,
})

// this test checks that apps can pass methods as arguments to other methods and have them still work
const hello = contract.hello

test("hello", async (t) => {
  t.deepEqual((await hello({ world: "tests" })).result, ["Hello", "tests"]);
});
