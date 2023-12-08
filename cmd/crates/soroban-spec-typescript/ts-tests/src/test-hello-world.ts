import test from "ava";
import { wallet, publicKey, rpcUrl } from "./util.js";
import { Address, Contract, networks } from "test-hello-world";

const contract = new Contract({...networks.standalone, rpcUrl, wallet});

test("hello", async (t) => {
  t.deepEqual(await contract.hello({ world: "tests" }), ["Hello", "tests"]);
});

// Currently must run tests in serial because nonce logic not smart enough to handle concurrent calls.
test.serial.failing("auth", async (t) => {
  t.deepEqual(await contract.auth({ addr: publicKey, world: 'lol' }), Address.fromString(publicKey))
});

test.serial.failing("inc", async (t) => {
  t.is(await contract.getCount(), 0);
  t.is(await contract.inc(), 1)
  t.is(await contract.getCount(), 1);
});
