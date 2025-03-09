import { GameServerWriteClient, type Match } from "gn-matchmaker-client";
import { sleep } from "bun";
import * as amqplib from "amqplib";
import type { Task } from "./types";
import SchnapsenClient from "gn-schnapsen-client";
import {
  initDefaultState,
  intoStateCard,
  schnapsenPredict,
  type State,
} from "./ai-routes";

const AI_TASK_QUEUE = "ai-task-generate-request";
const AI_REGISTER_QUEUE = "ai-register";

amqplib.connect(process.env.AMQP_URL!).then(async (conn) => {
  let channel = await conn.createChannel();
  channel.assertQueue(AI_TASK_QUEUE, { durable: false });
  channel.assertQueue(AI_REGISTER_QUEUE, { durable: false });

  let bugo_hoss = {
    game: "Schnapsen",
    elo: 250,
    display_name: "Bugo Hoss",
  };

  let lalph_raulen = {
    game: "Schnapsen",
    elo: 500,
    display_name: "Lalph Raulen",
  };

  let kolfgang_woscher = {
    game: "Schnapsen",
    elo: 1500,
    display_name: "Kolfgang Woscher",
  };

  let bugo_hoss_id = bugo_hoss.display_name;
  let lalph_raulen_id = lalph_raulen.display_name;
  let kolfgang_woscher_id = kolfgang_woscher.display_name;

  function registerAIFor(info: any, modes: string[]) {
    modes.forEach((mode) => {
      channel.publish("", AI_REGISTER_QUEUE, Buffer.from(JSON.stringify({ ...info, mode })));
    });
  }

  registerAIFor(bugo_hoss, ["speed", "bummerl"])
  registerAIFor(lalph_raulen, ["speed", "bummerl"])
  registerAIFor(kolfgang_woscher, ["speed", "bummerl"])

  channel.consume(AI_TASK_QUEUE, async (msg) => {
    let stop = false;
    let played_card = false

    if (msg === null) {
      return;
    }

    let task: Task = JSON.parse(msg.content.toString());

    if (task.game !== "Schnapsen") {
      channel.nack(msg);
      return;
    }

    channel.ack(msg);

    let state: State = initDefaultState();
    let ai_level = 4

    switch (task.ai_id) {
      case bugo_hoss_id:
        state.ki_level = 4;
        break;
      case lalph_raulen_id:
        state.ki_level = 4;
        break;
      case kolfgang_woscher_id:
        state.ki_level = 4;
        break;
    }

    state.ki_level = ai_level;

    task.address = `http://${task.address}`

    console.log(task.address)
    let client = new SchnapsenClient(task.write, task as Match);

    console.log("Client initialized for match", task.read);

    client.on("self:allow_announce", async () => {
      stop = true
      await sleep(1000)

      if (played_card || client.announceable![0] === undefined) {
        stop = false
        return;
      }

      const announcement = client.announceable![0];
      console.log(announcement)
      if (announcement.data.announce_type == "Forty") {
        client.announce40();
      } else {
        client.announce20(announcement.data.cards);
      }

      await sleep(1000)
      client.playCard(announcement.data.cards[0]);
      await sleep(1000)
      stop = false
    });

    client.on("round_result", async (result) => {
      state = initDefaultState();
      state.ki_level = ai_level;
    })

    client.on("self:trump_change_possible", async (card) => {
      let onSwap = async () => {
        await sleep(500)
        // @ts-ignore
        state[intoStateCard(card.data) as keyof State] = 0;
        client.swapTrump(card.data);
        client.off("self:allow_swap_trump", onSwap)
      }

      client.on("self:allow_swap_trump", onSwap)
    });

    let tried_replay = false;
    const replay_cap = 10;
    client.on("error", async (error) => {
      if (tried_replay) {
        console.error("Fatal ERROR")
        return;
      }
      tried_replay = true;
      await sleep(1000)
      client.playCard(
        client.cardsPlayable[
          Math.floor(Math.random() * client.cardsPlayable.length)
        ]
      );
      console.error(error);
      console.log("Error occured, playing random card");
    })

    client.on("reset", async () => {
      tried_replay = false;
    })


    client.on("self:allow_play_card", async () => {
      played_card = false
      console.log("Playing Card")
        await sleep(1300)

        if (stop) {
          return;
        }

        played_card = true

        if (client.deckCardCount == 0) {
          state.follow_suit = true
        }

        let card = await schnapsenPredict(state);

        if (card.suit == "[ilegal values]" || !client.cardsPlayable.some(e => e.suit == card.suit && e.value == card.value)) {
          console.log("Had illegal values with state: ", state)
          client.playCard(
            client.cardsPlayable[
              Math.floor(Math.random() * client.cardsPlayable.length)
            ]
          );
        } else {
          client.playCard(card);
        }
    });

    client.on("trump_change", async (trump) => {
      console.log("Trump changed to", trump.card);
      if (trump.card !== null) {
        state.trump_suit = intoStateCard(trump.card);
        console.log("Trump changed to", state.trump_suit);
      }
    })

    client.on("play_card", async (event) => {
      // @ts-ignore
      // state[intoStateCard(event.data.card) as keyof State] = 2;

      if (event.data.user_id === client.userId) {
        state.played_card_by_opponent = "No_Card";
        return;
      }
      state.played_card_by_opponent = intoStateCard(event.data.card);
    })

    client.on("trick", async (data) => {
      state.played_card_by_opponent = "No_Card";
    })

    client.on("close_talon", async () => {
      state.follow_suit = true;
    })

    client.on("self:card_available", async (card) => {
      // @ts-ignore
      state[intoStateCard(card.data) as keyof State] = 1;
    });

    client.on("self:card_unavailable", async (card) => {
      // @ts-ignore
      // state[intoStateCard(card.data) as keyof State] = 2;
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
