import { grpcClient } from "./grpc_client";
import { sleep } from "./util";
import { strict as assert } from "assert";

async function main() {
  try {
    const result = await grpcClient.client.L2BlocksQuery({ limit: 3 });
    console.log(result);
  } catch (error) {
    console.error("Caught error:", error);
    process.exit(1);
  }
}

main();
