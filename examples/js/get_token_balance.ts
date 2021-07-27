import { grpcClient } from "./grpc_client";
import { kafkaProducer } from "./kafka_producer";
import { sleep } from "./util";
import { strict as assert } from "assert";

const tokenId = 0;
const userId = 0;

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

  await getTokenBalanceTest();
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
      key: "balances",
      value: JSON.stringify(kafkaBalanceValue)
    }
  ]);
}

async function getTokenBalanceTest() {
  const res = await grpcClient.tokenBalanceQuery(userId, tokenId, "", "");
  assert.equal(res["balance"], "3.0000");
  assert.equal(res["balance_raw"], "30000");
  assert.equal(res["precision"], 4);

  console.log("getTokenBalanceTest passed");
}

main();
