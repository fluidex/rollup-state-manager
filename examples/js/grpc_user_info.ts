import { grpcClient } from "./grpc_client";
import { strict as assert } from "assert";

const userInfo1 = {
  userId: 1,
  l1Address: "0x6286d0A2FC1d4C12a4ACc274018b401c68157Fdb",
  l2Pubkey: "0x5d182c51bcfe99583d7075a7a0c10d96bef82b8a059c4bf8c5f6e7124cf2bba3"
};

const userInfo2 = {
  userId: 2,
  l1Address: "0xf40e08f651f2f7f96f5602114e5a77f1a7beea5d",
  l2Pubkey: "0xe9b54eb2dbf0a14faafd109ea2a6a292b78276c8381f8ef984dddefeafb2deaf"
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
  await registerUsers();
  await getUserInfos();
}

async function registerUsers() {
  console.log("registerUsers Begin");

  // Register user 1.
  let res = await grpcClient.registerUser(
    userInfo1.userId,
    userInfo1.l1Address,
    userInfo1.l2Pubkey
  );

  res = res["user_info"];
  assert.equal(res["user_id"], userInfo1.userId);
  assert.equal(res["l1_address"], userInfo1.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo1.l2Pubkey.toLowerCase());

  // Register user 2.
  res = await grpcClient.registerUser(
    userInfo2.userId,
    userInfo2.l1Address,
    userInfo2.l2Pubkey
  );

  res = res["user_info"];
  assert.equal(res["user_id"], userInfo2.userId);
  assert.equal(res["l1_address"], userInfo2.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo2.l2Pubkey.toLowerCase());

  console.log("registerUsers End");
}

async function getUserInfos() {
  console.log("getUserInfos Begin");

  // Get user 1 by user ID.
  let res = await grpcClient.userInfoQuery(userInfo1.userId, null, null);
  res = res["user_info"];
  assert.equal(res["user_id"], userInfo1.userId);
  assert.equal(res["l1_address"], userInfo1.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo1.l2Pubkey.toLowerCase());

  // Get user 1 by L1 address.
  res = await grpcClient.userInfoQuery(null, userInfo1.l1Address, null);
  res = res["user_info"];
  assert.equal(res["user_id"], userInfo1.userId);
  assert.equal(res["l1_address"], userInfo1.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo1.l2Pubkey.toLowerCase());

  // Get user 1 by L2 public key.
  res = await grpcClient.userInfoQuery(null, null, userInfo1.l2Pubkey);
  res = res["user_info"];
  assert.equal(res["user_id"], userInfo1.userId);
  assert.equal(res["l1_address"], userInfo1.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo1.l2Pubkey.toLowerCase());

  // Get user 2 by user ID.
  res = await grpcClient.userInfoQuery(userInfo2.userId, null, null);
  res = res["user_info"];
  assert.equal(res["user_id"], userInfo2.userId);
  assert.equal(res["l1_address"], userInfo2.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo2.l2Pubkey.toLowerCase());

  // Get user 2 by L1 address.
  res = await grpcClient.userInfoQuery(null, userInfo2.l1Address, null);
  res = res["user_info"];
  assert.equal(res["user_id"], userInfo2.userId);
  assert.equal(res["l1_address"], userInfo2.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo2.l2Pubkey.toLowerCase());

  // Get user 2 by L2 public key.
  res = await grpcClient.userInfoQuery(null, null, userInfo2.l2Pubkey);
  res = res["user_info"];
  assert.equal(res["user_id"], userInfo2.userId);
  assert.equal(res["l1_address"], userInfo2.l1Address.toLowerCase());
  assert.equal(res["l2_pubkey"], userInfo2.l2Pubkey.toLowerCase());

  console.log("getUserInfos End");
}

main();
