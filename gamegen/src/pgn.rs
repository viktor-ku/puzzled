#[derive(Debug)]
pub struct Pgn {
    pub result: String,
    pub pgn: String,
}

impl Pgn {
    pub fn with(moves: Vec<String>, result: Outcome) -> Self {
        let mut pgn = String::new();
        let outcome_str = &result.to_string();

        for (i, pair) in moves.chunks_exact(2).enumerate() {
            let move_nr = i + 1;
            let white = pair.get(0).unwrap();
            let black = pair.get(1);

            let mut fullmove = format!("{}. {}", move_nr, white);
            if let Some(black) = black {
                fullmove.push_str(&format!(" {}", black));
            }

            pgn.push_str(&format!("{} ", fullmove));
        }

        pgn.push_str(outcome_str);

        Self {
            result: outcome_str.to_string(),
            pgn,
        }
    }
}

