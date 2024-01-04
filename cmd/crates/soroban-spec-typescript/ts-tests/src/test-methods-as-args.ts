import test from "ava";
import { wallet, rpcUrl } from "./util.js";
import { Contract, networks } from "test-hello-world";

const contract = new Contract({ ...networks.standalone, rpcUrl, wallet });

// this test checks that apps can pass methods as arguments to other methods and have them still work
const hello = contract.hello

test("hello", async (t) => {
  t.deepEqual((await hello({ world: "tests" })).result, ["Hello", "tests"]);
});
