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

  let block = res["blocks"][0];
  assert.equal(block["block_height"], "2");
  assert(block["block_time"] <= time_now_milliseconds);
  assert(block["block_time"] + one_hour_milliseconds > time_now_milliseconds);
  assert.equal(
    block["merkle_root"],
    "0x0eb470af91c202fc21b920dde57857a60483e298be687df030ba1096bad52f36"
  );

  block = res["blocks"][1];
  assert.equal(block["block_height"], "1");
  assert(block["block_time"] <= time_now_milliseconds);
  assert(block["block_time"] + one_hour_milliseconds > time_now_milliseconds);
  assert.equal(
    block["merkle_root"],
    "0x1ab8107bab6aa9ca2ccab519821547379375d3266184c7490c4fd07699d0dcb7"
  );

  console.log("getL2BlocksTest passed");
}

main();
