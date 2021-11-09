import * as dayjs from "dayjs";
import { grpcClient } from "./grpc_client";
import { kafkaProducer } from "./kafka_producer";
import { sleep } from "./util";
import { strict as assert } from "assert";

const one_hour_milliseconds = 3_600_000;

const blockId = 0;
const userId1 = 1;
const userId2 = 2;

const userMsg1 = {
  user_id: userId1,
  l1_address: "0x6286d0A2FC1d4C12a4ACc274018b401c68157Fdb",
  l2_pubkey:
    "0x5d182c51bcfe99583d7075a7a0c10d96bef82b8a059c4bf8c5f6e7124cf2bba3"
};

const userMsg2 = {
  user_id: userId2,
  l1_address: "0xf40e08f651f2f7f96f5602114e5a77f1a7beea5d",
  l2_pubkey:
    "0xe9b54eb2dbf0a14faafd109ea2a6a292b78276c8381f8ef984dddefeafb2deaf"
};

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

  await registerUsers();
  await depositBalance();
  await sleep(3000);
  await kafkaProducer.Stop();

  await testBlock0();
  await testBlock1();
}

async function registerUsers() {
  await kafkaProducer.send([
    {
      key: "registeruser",
      value: JSON.stringify(userMsg1)
    },
    {
      key: "registeruser",
      value: JSON.stringify(userMsg2)
    }
  ]);
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
    "0x0eb470af91c202fc21b920dde57857a60483e298be687df030ba1096bad52f36"
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
    "0x1ab8107bab6aa9ca2ccab519821547379375d3266184c7490c4fd07699d0dcb7"
  );
  assert.equal(res["txs"].length, 2);
  assert.equal(res["decoded_txs"].length, 2);
  assert.deepEqual(res["txs_type"], ["DEPOSIT", "WITHDRAW"]);

  const tx1 = res["decoded_txs"][0]["deposit_tx"];
  assert.equal(tx1["account_id"], userId1);
  assert.equal(tx1["token_id"], 1);
  assert.equal(tx1["amount"], "500000");
  assert.equal(tx1["old_balance"], "0.000000");
  assert.equal(tx1["new_balance"], "500000.000000");

  const tx2 = res["decoded_txs"][1]["withdraw_tx"];
  assert.equal(tx2["account_id"], userId1);
  assert.equal(tx2["token_id"], 1);
  assert.equal(tx2["amount"], "100");
  assert.equal(tx2["old_balance"], "500000.000000");
  assert.equal(tx2["new_balance"], "499900.000000");

  console.log("testBlock1 End");
}

main();
