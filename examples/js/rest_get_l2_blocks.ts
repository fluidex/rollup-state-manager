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
    "0x2db7473a800f079e86d214eb16c8a0d85cb4c4b172dab43c1a493e1132bdb312"
  );

  console.log("getL2BlocksTest passed");
}

main();
