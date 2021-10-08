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
  assert.equal(res["total"], "1");

  const block = res["blocks"][0];
  assert.equal(block["block_height"], "1");
  assert.equal(
    block["merkle_root"],
    "0x1f237e688aef284ee8df484f0787334a06ce4ed490bfd868bc2c581a7eaa9c8c"
  );

  console.log("getL2BlocksTest passed");
}

main();
