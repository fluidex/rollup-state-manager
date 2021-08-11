import * as Kafka from "kafkajs";

class Producer {
  producer: any;

  async Init() {
    const brokers = process.env.KAFKA_BROKERS;
    const kafka = new Kafka.Kafka({
      brokers: (brokers || "127.0.0.1:9092").split(","),
      logLevel: Kafka.logLevel.WARN
    });
    const producer = kafka.producer();
    this.producer = producer;
    await producer.connect();
  }

  async send(messages, topic = "unifyevents") {
    await this.producer.send({
      topic,
      messages
    });
  }

  async Stop() {
    await this.producer.disconnect();
  }
}

let kafkaProducer = new Producer();
export { Producer, kafkaProducer };
