import * as dayjs from "dayjs";
import { restClient } from "./rest_client";
import { strict as assert } from "assert";

const one_hour_milliseconds = 3_600_000;

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
  assert.equal(res["total"], "3");

  const time_now_milliseconds = dayjs().valueOf();

  //notice the arry of blocks start from the latest block
  let block = res["blocks"][0];
  assert.equal(block["block_height"], "2");
  assert(block["block_time"] <= time_now_milliseconds);
  assert(block["block_time"] + one_hour_milliseconds > time_now_milliseconds);
  assert.equal(
    block["merkle_root"],
    "0x157b359e2fed778742b7f42f6e438d6552215f86473ac5b668a7ce3799062a61"
  );

  block = res["blocks"][1];
  assert.equal(block["block_height"], "1");
  assert(block["block_time"] <= time_now_milliseconds);
  assert(block["block_time"] + one_hour_milliseconds > time_now_milliseconds);
  assert.equal(
    block["merkle_root"],
    "0x0cf9708094c494c668f6943ab4cfba04882d2b25303244e6ae6f14931a0c008c"
  );

  console.log("getL2BlocksTest passed");
}

main();
