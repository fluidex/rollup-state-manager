import axios, {AxiosInstance} from "axios";

class Client {
    client: AxiosInstance;

    constructor(server = process.env.REST_ADDR || "http://127.0.0.1:8766/api") {
        console.log("using REST", server);
        this.client = axios.create({ baseURL: server, timeout: 1000 });
    }

    async l2BlocksQuery() {
        let resp = await this.client.get(`/l2_blocks`);
        if (resp.status === 200) {
            return resp.data
        } else {
            throw new Error(`request failed with ${resp.status} ${resp.statusText}`)
        }
    }
}

let restClient = new Client();
export { Client, restClient };
