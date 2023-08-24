import test from "ava";
import { wallet, publicKey, rpcUrl } from "./util.js";
import { Address, Contract, networks } from "test-hello-world";

const addr = Address.fromString(publicKey);

const contract = new Contract({...networks.standalone, rpcUrl, wallet});

test("hello", async (t) => {
  t.deepEqual(await contract.hello({ world: "tests" }), ["Hello", "tests"]);
});

// Currently must run tests in serial because nonce logic not smart enough to handle concurrent calls.
test.serial("auth", async (t) => {
  t.deepEqual(await contract.auth({ addr, world: 'lol' }), addr)
});

test.serial("inc", async (t) => {
  t.is(await contract.getCount(), 0);
  t.is(await contract.inc({}), 1)
  t.is(await contract.getCount(), 1);
});
