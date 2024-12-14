import { GameServerWriteClient, type Match } from "gn-matchmaker-client";
import * as amqplib from "amqplib";
import type { Task } from "./types";
import SchnapsenClient from "gn-schnapsen-client";
import {
  initDefaultState,
  intoStateCard,
  schnapsenPredict,
  type State,
} from "./ai-routes";

const AI_QUEUE = "ai-task-generate-request";

amqplib.connect(process.env.AMQP_URL!).then(async (conn) => {
  let channel = await conn.createChannel();
  channel.assertQueue(AI_QUEUE, { durable: false });

  channel.consume(AI_QUEUE, async (msg) => {
    if (msg === null) {
      return;
    }

    let task: Task = JSON.parse(msg.content.toString());

    if (task.game !== "Schnapsen" || task.mode !== "duo") {
      channel.nack(msg);
      return;
    }

    channel.ack(msg);

    let state: State = initDefaultState();
    state.ki_level = task.ai_level;

    task.address = `http://${task.address}`
    let client = new SchnapsenClient(task.write, task as Match);

    console.log("Client initialized for match", task.read);


    client.on("timeout", async (timeout: any) => {
      console.log("Timeout: ", timeout);
    })

    client.on("finished_distribution", async () => {
      // TODO: set trump suite
    });

    client.on("self:allow_announce", async () => {
      return;
      const announcement = client.announceable![0];
      if (announcement.data.announce_type == "Forty") {
        client.announce40();
      } else {
        client.announce20(announcement.data.cards);
      }
    });

    client.on("self:trump_change_possible", async (card) => {
      client.swapTrump(card.data);
    });

    client.on("self:allow_play_card", async () => {
      console.log("Playing Card")
        let force_color_guess = client.cardsPlayable.every((card) => card.suit == client.cardsPlayable[0].suit);
        state.follow_suit = force_color_guess;

        let card = await schnapsenPredict(state);
        console.log("AI predicted: ", card);
        client.playCard(card);
    });

    client.on("self:allow_draw_card", async () => {
      client.drawCard();
    });

    client.on("self:card_available", async (card) => {
      // @ts-ignore
      state[intoStateCard(card.data) as keyof State] = 1;
    });

    client.on("self:card_unavailable", async (card) => {
      // @ts-ignore
      state[intoStateCard(card.data) as keyof State] = 0;
    });

    client.on("trick", async (trick) => {
        trick.data.cards.forEach((card) => {
            // @ts-ignore
            state[intoStateCard(card) as keyof State] = 2;
        });
    })

    client.on("score", async (score) => {
      if (score.data.user_id !== client.userId) {
        state.my_points = score.data.points;
      } else {
        state.opponent_points = score.data.points;
      }
    });
  });
});
