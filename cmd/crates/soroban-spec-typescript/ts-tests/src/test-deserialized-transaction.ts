import test from "ava";
import { rpcUrl, root, signer } from "./util.js";
import { Client, networks } from "test-custom-types";

const contract = new Client({
  ...networks.local,
  rpcUrl,
  allowHttp: true,
  publicKey: root.keypair.publicKey(),
  ...signer,
});

test("has correctly-typed result", async (t) => {
  const initial = await contract.hello({ hello: "tests" });
  t.is(initial.result, "tests");

  const serialized = initial.toJSON();
  const deserialized = contract.fromJSON.hello(serialized);
  t.is(deserialized.result, "tests"); // throws TS error if `result` is of type `unknown`
});
