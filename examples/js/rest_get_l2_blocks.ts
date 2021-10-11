import { restClient } from "./rest_client";
import { strict as assert } from "assert";

async function main() {
  try {
    await mainTest();
  } catch (error) {
    console.error("Caught error:", error);
    process.exit(1);
  }
}

async function mainTest() {
  await getL2BlocksTest();
}

async function getL2BlocksTest() {
  const res = await restClient.l2BlocksQuery();
  assert.equal(res["total"], "2");

  let block = res["blocks"][0];
  assert.equal(block["block_height"], "1");
  assert.equal(
    block["merkle_root"],
    "0x1f237e688aef284ee8df484f0787334a06ce4ed490bfd868bc2c581a7eaa9c8c"
  );

  block = res["blocks"][1];
  assert.equal(block["block_height"], "0");
  assert.equal(
    block["merkle_root"],
    "0x29b6ba8438d7a56e30c8946cf2b7c8ed2b8db52cc64f1f4840b215209c3c593c"
  );

  console.log("getL2BlocksTest passed");
}

main();
