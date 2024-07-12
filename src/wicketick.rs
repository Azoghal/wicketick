// Simple summary just stores one innings, for now
pub struct SimpleSummary {
    pub current_innings: Innings,
}

impl SimpleSummary {
    pub fn display(&self) -> String {
        return self.current_innings.display();
    }
}

pub struct Innings {
    pub runs: i32,
    pub wickets: i32,
    pub overs: String,
}

impl Innings {
    pub fn display(&self) -> String {
        let runs = self.runs;
        let wickets = self.wickets;
        let overs = &self.overs;
        let text = format!("{}-{} {}", runs, wickets, overs);
        return text;
    }
}
