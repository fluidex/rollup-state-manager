import * as caller from "@eeston/grpc-caller";

const file = "../../proto/rollup_state.proto";
const load = {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true
};

class Client {
  client: any;

  constructor(server = process.env.GRPC_ADDR || "127.0.0.1:50061") {
    console.log("using GRPC", server);
    this.client = caller(`${server}`, { file, load }, "RollupState");
  }

  async tokenBalanceQuery(account_id, token_id, token_address, token_name): Promise<Map<string, any>> {
    return (await this.client.tokenBalanceQuery({
      account_id,
      token_id,
      token_address,
      token_name,
    }));
  }
}

let grpcClient = new Client();
export { Client, grpcClient };
