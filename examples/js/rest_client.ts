import axios, { AxiosInstance } from "axios";

class Client {
  client: AxiosInstance;

  constructor(
    server = process.env.REST_ADDR || "http://0.0.0.0:8765/explorer/api"
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
}

let restClient = new Client();
export { Client, restClient };
