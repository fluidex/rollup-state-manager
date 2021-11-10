// This test must be run after `get_token_balance.ts`. Since the test cases use
// the previous build Kafka data.

import { restClient } from "./rest_client";
import { strict as assert } from "assert";

const tokenId = 0;
const userId = 3;

async function main() {
  try {
    await mainTest();
  } catch (error) {
    console.error("Caught error:", error);
    process.exit(1);
  }
}

async function mainTest() {
  await testGetTokenBalanceByTokenId();
  await testGetTokenBalanceByTokenName();
}

async function testGetTokenBalanceByTokenId() {
  console.log("testGetTokenBalanceByTokenId Begin");

  const res = await restClient.tokenBalanceQuery(userId, tokenId, null, null);
  assert.equal(res["balance"], "3.0000");
  assert.equal(res["balance_raw"], "30000");
  assert.equal(res["precision"], 4);

  console.log("testGetTokenBalanceByTokenId End");
}

async function testGetTokenBalanceByTokenName() {
  console.log("testGetTokenBalanceByTokenName Begin");

  const res = await restClient.tokenBalanceQuery(userId, null, null, "ETH");
  assert.equal(res["balance"], "3.0000");
  assert.equal(res["balance_raw"], "30000");
  assert.equal(res["precision"], 4);

  console.log("testGetTokenBalanceByTokenName End");
}

main();
