// TODO:
// Refactor to export User Info functions as common. For now, this script needs
// to be run after `grpc_user_info.ts`.

require("dotenv").config();

import * as dayjs from "dayjs";
import { grpcClient } from "./grpc_client";
import { kafkaProducer } from "./kafka_producer";
import { sleep } from "./util";
import { strict as assert } from "assert";

const one_hour_milliseconds = 3_600_000;

const userId1 = 1;
const userId2 = 2;

const depositMsg = {
  timestamp: 16264463600,
  user_id: userId1,
  asset: "USDT",
  business: "deposit",
  change: "500000",
  balance: "500000",
  balance_available: "500000",
  balance_frozen: "0",
  detail: JSON.stringify({ user_id: userId1 })
};

const withdrawMsg = {
  timestamp: 1631097274,
  user_id: userId1,
  asset: "USDT",
  business: "withdraw",
  change: "-100",
  balance: "499900.0",
  balance_available: "499900.0",
  balance_frozen: "0",
  detail: JSON.stringify({ id: 1631097274567, key0: "value0" }),
  signature:
    "ce6e16056da007b3d7274db0ef3f546a101bd99be4137dfe97340b8f2481caac3a7af82bef9d00c226227280413de24714acd2458ba369d052ea43e0cd063601"
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

  await depositBalance();
  await sleep(3000);
  await kafkaProducer.Stop();

  await testBlock0();
  await testBlock1();
}

async function depositBalance() {
  await kafkaProducer.send([
    {
      key: "deposits",
      value: JSON.stringify(depositMsg)
    },
    {
      key: "withdraws",
      value: JSON.stringify(withdrawMsg)
    }
  ]);
}

async function testBlock0() {
  console.log("testBlock0 Begin");

  const res = await grpcClient.l2BlockQuery(0);
  assert.equal(res["tx_num"], "2");
  assert.equal(res["real_tx_num"], "2");
  const time_now_milliseconds = dayjs().valueOf();
  assert(res["created_time"] <= time_now_milliseconds);
  assert(res["created_time"] + one_hour_milliseconds > time_now_milliseconds);
  assert.equal(res["status"], "UNCOMMITED");
  assert.equal(
    res["new_root"],
    "0x1ab8107bab6aa9ca2ccab519821547379375d3266184c7490c4fd07699d0dcb7"
  );
  assert.equal(res["txs"].length, 2);
  assert.equal(res["decoded_txs"].length, 2);
  assert.deepEqual(res["txs_type"], ["DEPOSIT", "DEPOSIT"]);

  const tx1 = res["decoded_txs"][0]["deposit_tx"];
  assert.equal(tx1["account_id"], userId1);
  assert.equal(tx1["token_id"], 0);
  assert.equal(tx1["amount"], "0.0000");
  assert.equal(tx1["old_balance"], "0.0000");
  assert.equal(tx1["new_balance"], "0.0000");

  const tx2 = res["decoded_txs"][1]["deposit_tx"];
  assert.equal(tx2["account_id"], userId2);
  assert.equal(tx2["token_id"], 0);
  assert.equal(tx2["amount"], "0.0000");
  assert.equal(tx2["old_balance"], "0.0000");
  assert.equal(tx2["new_balance"], "0.0000");

  console.log("testBlock0 End");
}

async function testBlock1() {
  console.log("testBlock1 Begin");

  const res = await grpcClient.l2BlockQuery(1);
  assert.equal(res["tx_num"], "2");
  assert.equal(res["real_tx_num"], "2");
  const time_now_milliseconds = dayjs().valueOf();
  assert(res["created_time"] <= time_now_milliseconds);
  assert(res["created_time"] + one_hour_milliseconds > time_now_milliseconds);
  assert.equal(res["status"], "UNCOMMITED");
  assert.equal(
    res["new_root"],
    "0x0cf9708094c494c668f6943ab4cfba04882d2b25303244e6ae6f14931a0c008c"
  );
  assert.equal(res["txs"].length, 2);
  assert.equal(res["decoded_txs"].length, 2);
  assert.deepEqual(res["txs_type"], ["DEPOSIT", "WITHDRAW"]);

  const tx1 = res["decoded_txs"][0]["deposit_tx"];
  assert.equal(tx1["account_id"], userId1);
  assert.equal(tx1["token_id"], 1);
  assert.equal(tx1["amount"], "500000.000000");
  assert.equal(tx1["old_balance"], "0.000000");
  assert.equal(tx1["new_balance"], "500000.000000");

  const tx2 = res["decoded_txs"][1]["withdraw_tx"];
  assert.equal(tx2["account_id"], userId1);
  assert.equal(tx2["token_id"], 1);
  assert.equal(tx2["amount"], "100.000000");
  assert.equal(tx2["old_balance"], "500000.000000");
  assert.equal(tx2["new_balance"], "499900.000000");

  console.log("testBlock1 End");
}

main();
