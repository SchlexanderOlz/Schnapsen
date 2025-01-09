import { types } from "gn-schnapsen-client";

export interface State {
  Hearts_J: number;
  Hearts_Q: number;
  Hearts_K: number;
  Hearts_T: number;
  Hearts_A: number;
  Acorns_J: number;
  Acorns_Q: number;
  Acorns_K: number;
  Acorns_T: number;
  Acorns_A: number;
  Leaves_J: number;
  Leaves_Q: number;
  Leaves_K: number;
  Leaves_T: number;
  Leaves_A: number;
  Bells_J: number;
  Bells_Q: number;
  Bells_K: number;
  Bells_T: number;
  Bells_A: number;
  trump_suit: string;
  played_card_by_opponent: string;
  follow_suit: boolean;
  my_points: number;
  opponent_points: number;
  ki_level: number;
}

export const initDefaultState = (): State => {
  return {
    Hearts_J: 0,
    Hearts_Q: 0,
    Hearts_K: 0,
    Hearts_T: 0,
    Hearts_A: 0,
    Acorns_J: 0,
    Acorns_Q: 0,
    Acorns_K: 0,
    Acorns_T: 0,
    Acorns_A: 0,
    Leaves_J: 0,
    Leaves_Q: 0,
    Leaves_K: 0,
    Leaves_T: 0,
    Leaves_A: 0,
    Bells_J: 0,
    Bells_Q: 0,
    Bells_K: 0,
    Bells_T: 0,
    Bells_A: 0,
    trump_suit: "No_Card",
    played_card_by_opponent: "No_Card",
    follow_suit: false,
    my_points: 0,
    opponent_points: 0,
    ki_level: 0,
  };
};

export const intoStateCard = (card: types.Card): string => {
  let suit = card.suit as string;

  switch (card.suit) {
    case "Diamonds":
      suit = "Bells";
      break;
    case "Spades":
      suit = "Leaves";
      break;
    case "Clubs":
      suit = "Acorns";
      break;
  }

  return suit + "_" + card.value[0];
};

export const fromStateCard = (card: string): types.Card => {
  let suit = card.split("_")[0];
  switch (suit) {
    case "Bells":
      suit = "Diamonds";
      break;
    case "Leaves":
      suit = "Spades";
      break;
    case "Acorns":
      suit = "Clubs";
      break;
  }

  let value = card.split("_")[1];

  switch (value) {
    case "J":
      value = "Jack";
      break;
    case "Q":
      value = "Queen";
      break;
    case "K":
      value = "King";
      break;
    case "T":
      value = "Ten";
      break;
    case "A":
      value = "Ace";
      break;
  }

  return {
    suit: suit as types.CardSuit,
    value: value as types.CardVal,
  };
};

const AI_MODEL_URL = process.env.SCHNAPSEN_AI_MODEL_URL!;
const AI_MODEL_TOKEN = process.env.SCHNAPSEN_AI_TOKEN!;

export const schnapsenPredict = async (state: State): Promise<types.Card> => {
  const card = await fetch(AI_MODEL_URL, {
    method: "POST",
    body: JSON.stringify(state),
    headers: {
      "Content-Type": "application/json",
      "x-token": AI_MODEL_TOKEN
    },
  }).then(res => res.text()).catch((err) => {
    console.log(err);
    return "[ilegal values]";
  });

  return fromStateCard((card).replaceAll('"', ""));
};
