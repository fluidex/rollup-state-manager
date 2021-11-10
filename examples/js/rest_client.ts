import axios, { AxiosInstance } from "axios";

class Client {
  client: AxiosInstance;

  constructor(
    server = process.env.REST_ADDR || "http://0.0.0.0:8765/api/explorer"
  ) {
    console.log("using REST", server);
    this.client = axios.create({ baseURL: server, timeout: 1000 });
  }

  async l2BlocksQuery() {
    const offset = 0;
    const limit = 10;
    let resp = await this.client.get(
      `/l2_blocks?offset=${offset}&limit=${limit}`
    );
    if (resp.status === 200) {
      return resp.data;
    } else {
      throw new Error(`request failed with ${resp.status} ${resp.statusText}`);
    }
  }

  async tokenBalanceQuery(accountId, tokenId, tokenAddress, tokenName) {
    let params = null;
    if (tokenId != null) {
      params = `token_id=${tokenId}`;
    } else if (tokenAddress != null) {
      params = `token_address=${tokenAddress}`;
    } else if (tokenName != null) {
      params = `token_name=${tokenName}`;
    }

    let resp = await this.client.get(`/token_balance/${accountId}?${params}`);
    if (resp.status === 200) {
      return resp.data;
    } else {
      throw new Error(`request failed with ${resp.status} ${resp.statusText}`);
    }
  }
}

let restClient = new Client();
export { Client, restClient };
