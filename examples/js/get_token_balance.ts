require("dotenv").config();

import { grpcClient } from "./grpc_client";
import { kafkaProducer } from "./kafka_producer";
import { sleep } from "./util";
import { strict as assert } from "assert";

const tokenId = 0;
const userId = 3;

const kafkaUserValue = {
  user_id: userId,
  l1_address: "0x6286d0A2FC1d4C12a4ACc274018b401c68157Fdb",
  l2_pubkey:
    "0x5d182c51bcfe99583d7075a7a0c10d96bef82b8a059c4bf8c5f6e7124cf2bba3"
};

const kafkaBalanceValue = {
  timestamp: 16264463600,
  user_id: userId,
  asset: "ETH",
  business: "deposit",
  change: "3",
  balance: "3",
  balance_available: "3",
  balance_frozen: "0",
  detail: JSON.stringify({ id: 0 })
};

async function main() {
  try {
    await mainTest();
  } catch (error) {
    console.error("Caught error:", error);
    process.exit(1);
  }
}

async function mainTest() {
  await kafkaProducer.Init();

  await registerUser();
  await depositBalance();
  await sleep(1000);
  await kafkaProducer.Stop();

  await testGetTokenBalanceByTokenId();
  await testGetTokenBalanceByTokenName();

  await getL2BlockTest();
}

async function registerUser() {
  await kafkaProducer.send([
    {
      key: "registeruser",
      value: JSON.stringify(kafkaUserValue)
    }
  ]);
}

async function depositBalance() {
  await kafkaProducer.send([
    {
      key: "deposits",
      value: JSON.stringify(kafkaBalanceValue)
    }
  ]);
}

async function testGetTokenBalanceByTokenId() {
  console.log("testGetTokenBalanceByTokenId Begin");

  const res = await grpcClient.tokenBalanceQuery(userId, tokenId, null, null);
  assert.equal(res["balance"], "3.0000");
  assert.equal(res["balance_raw"], "30000");
  assert.equal(res["precision"], 4);

  console.log("testGetTokenBalanceByTokenId End");
}

async function testGetTokenBalanceByTokenName() {
  console.log("testGetTokenBalanceByTokenName Begin");

  const res = await grpcClient.tokenBalanceQuery(userId, null, null, "ETH");
  assert.equal(res["balance"], "3.0000");
  assert.equal(res["balance_raw"], "30000");
  assert.equal(res["precision"], 4);

  console.log("testGetTokenBalanceByTokenName End");
}

async function getL2BlockTest() {
  console.log("getL2BlockTest Begin");

  const res = await grpcClient.l2BlockQuery(2);
  assert.equal(res["tx_num"], "2");
  assert.equal(res["real_tx_num"], "2");
  assert(res["created_time"]);
  assert.equal(res["status"], "UNCOMMITED");
  assert.equal(
    res["new_root"],
    "0x157b359e2fed778742b7f42f6e438d6552215f86473ac5b668a7ce3799062a61"
  );
  assert.equal(res["txs"].length, 2);
  assert.equal(res["decoded_txs"].length, 2);
  assert.deepEqual(res["txs_type"], ["DEPOSIT", "DEPOSIT"]);

  const tx1 = res["decoded_txs"][0]["deposit_tx"];
  assert.equal(tx1["account_id"], userId);
  assert.equal(tx1["token_id"], 0);
  assert.equal(tx1["amount"], "0.0000");
  assert.equal(tx1["old_balance"], "0.0000");
  assert.equal(tx1["new_balance"], "0.0000");

  const tx2 = res["decoded_txs"][1]["deposit_tx"];
  assert.equal(tx2["account_id"], userId);
  assert.equal(tx2["token_id"], 0);
  assert.equal(tx2["amount"], "3.0000");
  assert.equal(tx2["old_balance"], "0.0000");
  assert.equal(tx2["new_balance"], "3.0000");

  console.log("getL2BlockTest End");
}

main();
