export interface Task {
    ai_id: string,
    game: string,
    mode: string,
    address: string,
    read: string,
    write: string,
    players: string[]
}

export interface AIPlayerRegister {
    game: string,
    mode: string,
    elo: number,
    display_name: string 
}