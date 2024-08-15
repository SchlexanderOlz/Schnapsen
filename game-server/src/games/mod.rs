use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchnapsenMode {
    TwoPlayer,
    FourPlayer,
}

impl SchnapsenMode {
    pub fn get_player_count(&self) -> u32 {
        match self {
            SchnapsenMode::TwoPlayer => 2,
            SchnapsenMode::FourPlayer => 4,
        }
    }
}

impl From<String> for SchnapsenMode {
    fn from(mode: String) -> Self {
        match mode.as_str() {
            "two_player" => SchnapsenMode::TwoPlayer,
            "four_player" => SchnapsenMode::FourPlayer,
            _ => panic!("Invalid Schnapsen mode: {}", mode),
        }
    }
}

pub enum SchnapsenType 
where Self: Hash 
{

    TwoPlayer(SchnapsenTwoPlayer),
    FourPlayer(SchnapsenFourPlayer),
}

impl Hash for SchnapsenType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            SchnapsenType::TwoPlayer(game) => game.hash(state),
            SchnapsenType::FourPlayer(game) => game.hash(state),
        }
    } 
}

impl Into<Box<dyn Schnapsen>> for SchnapsenType {
    fn into(self) -> Box<dyn Schnapsen> {
        self.into_schnapsen()
    }
}

impl SchnapsenType {
    pub fn into_schnapsen(self) -> Box<dyn Schnapsen> {
        match self {
            SchnapsenType::TwoPlayer(game) => Box::new(game),
            SchnapsenType::FourPlayer(game) => Box::new(game),
        }
    }
}


pub trait Schnapsen {
    fn has_player(&self, player: &str) -> bool;
}


pub struct SchnapsenTwoPlayer {
    players: [String; 2],
}

impl SchnapsenTwoPlayer {
    pub fn new(players: [String; 2]) -> Self {
        Self { players }
    }
}

impl Schnapsen for SchnapsenTwoPlayer {
    fn has_player(&self, player: &str) -> bool {
        self.players.contains(&player.to_string())
    }
}

pub struct SchnapsenFourPlayer {
    players: [String; 4],
}

impl SchnapsenFourPlayer {
    pub fn new(players: [String; 4]) -> Self {
        Self { players }
    }
}

impl Schnapsen for SchnapsenFourPlayer {
    fn has_player(&self, player: &str) -> bool {
        self.players.contains(&player.to_string())
    }
}

pub struct SchnapsenBuilder {
    mode: SchnapsenMode,
    players: Vec<String>,
}

impl SchnapsenBuilder {
    pub fn new() -> Self {
        Self {
            mode: SchnapsenMode::TwoPlayer,
            players: Vec::new(),
        }
    }

    pub fn mode(mut self, mode: SchnapsenMode) -> Self {
        if self.players.len() == 0 {
            self.mode = mode;
        }

        if mode == SchnapsenMode::TwoPlayer && self.players.len() != 2 {
            panic!("Two player mode requires exactly 2 players");
        } else if mode == SchnapsenMode::FourPlayer && self.players.len() != 4 {
            panic!("Four player mode requires exactly 4 players");
        }
        self
    }

    pub fn players(mut self, players: Vec<String>) -> Self {
        if players.len() == SchnapsenMode::FourPlayer.get_player_count() as usize {
            self.mode = SchnapsenMode::FourPlayer;
        } else if players.len() == SchnapsenMode::TwoPlayer.get_player_count() as usize {
            self.mode = SchnapsenMode::TwoPlayer;
        } else {
            panic!("Schnapsen cannot be played with {} players", players.len());
        }
        self.players = players;
        self
    }

    pub fn build(self) -> SchnapsenType {
        match self.mode {
            SchnapsenMode::TwoPlayer => SchnapsenType::TwoPlayer(SchnapsenTwoPlayer::new(
                self.players.try_into().unwrap(),
            )),
            SchnapsenMode::FourPlayer => SchnapsenType::FourPlayer(SchnapsenFourPlayer::new(self.players.try_into().unwrap())),
        }
    }
}

impl Hash for SchnapsenFourPlayer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.players.hash(state);
    }
}

impl Hash for SchnapsenTwoPlayer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.players.hash(state);
    }
}
